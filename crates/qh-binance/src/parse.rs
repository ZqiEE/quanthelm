use std::str::FromStr;

use chrono::{DateTime, Utc};
use qh_domain::{Price, Symbol};
use qh_market_data::{Interval, Kline, RawEvent};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::Value;

use crate::{
    BinanceError, ContractSpec, ContractType, ExchangeInfo, LotSizeFilter, PercentPriceFilter,
    PriceFilter, SymbolRules,
};

/// Parses an exact `/fapi/v1/exchangeInfo` payload.
pub fn parse_exchange_info(
    payload: &str,
    received_at: DateTime<Utc>,
) -> Result<ExchangeInfo, BinanceError> {
    let raw: RawExchangeInfo = serde_json::from_str(payload)
        .map_err(|error| BinanceError::InvalidResponse(error.to_string()))?;
    let server_time = Kline::timestamp(raw.server_time)?;
    let contracts = raw
        .symbols
        .into_iter()
        .map(parse_contract)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ExchangeInfo {
        server_time,
        contracts,
        raw_event: RawEvent::new(
            "binance-usdm",
            "/fapi/v1/exchangeInfo",
            Some(server_time),
            received_at,
            payload,
        ),
    })
}

/// Parses an exact `/fapi/v1/klines` payload.
pub fn parse_klines(
    payload: &str,
    symbol: Symbol,
    interval: Interval,
    observed_at: DateTime<Utc>,
) -> Result<Vec<Kline>, BinanceError> {
    let rows: Vec<Vec<Value>> = serde_json::from_str(payload)
        .map_err(|error| BinanceError::InvalidResponse(error.to_string()))?;
    rows.into_iter()
        .enumerate()
        .map(|(index, row)| parse_kline_row(index, &row, symbol.clone(), interval, observed_at))
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawExchangeInfo {
    server_time: i64,
    symbols: Vec<RawContract>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawContract {
    symbol: String,
    contract_type: String,
    status: String,
    base_asset: String,
    quote_asset: String,
    margin_asset: String,
    onboard_date: i64,
    filters: Vec<Value>,
}

fn parse_contract(raw: RawContract) -> Result<ContractSpec, BinanceError> {
    Ok(ContractSpec {
        symbol: Symbol::new(raw.symbol)?,
        contract_type: ContractType::from(raw.contract_type),
        status: raw.status.into(),
        base_asset: raw.base_asset,
        quote_asset: raw.quote_asset,
        margin_asset: raw.margin_asset,
        onboarded_at: Kline::timestamp(raw.onboard_date)?,
        rules: parse_filters(raw.filters)?,
    })
}

fn parse_filters(filters: Vec<Value>) -> Result<SymbolRules, BinanceError> {
    let mut rules = SymbolRules::default();
    for filter in filters {
        let filter_type = filter
            .get("filterType")
            .and_then(Value::as_str)
            .ok_or_else(|| BinanceError::InvalidResponse("filter missing filterType".to_owned()))?;
        match filter_type {
            "PRICE_FILTER" => {
                rules.price = Some(PriceFilter {
                    min_price: decimal_field(&filter, "minPrice", "PRICE_FILTER.minPrice")?,
                    max_price: decimal_field(&filter, "maxPrice", "PRICE_FILTER.maxPrice")?,
                    tick_size: decimal_field(&filter, "tickSize", "PRICE_FILTER.tickSize")?,
                });
            }
            "LOT_SIZE" => {
                rules.lot_size = Some(parse_lot_size(&filter, "LOT_SIZE")?);
            }
            "MARKET_LOT_SIZE" => {
                rules.market_lot_size = Some(parse_lot_size(&filter, "MARKET_LOT_SIZE")?);
            }
            "PERCENT_PRICE" => {
                rules.percent_price = Some(PercentPriceFilter {
                    multiplier_up: decimal_field(
                        &filter,
                        "multiplierUp",
                        "PERCENT_PRICE.multiplierUp",
                    )?,
                    multiplier_down: decimal_field(
                        &filter,
                        "multiplierDown",
                        "PERCENT_PRICE.multiplierDown",
                    )?,
                    multiplier_decimal: integer_field(
                        &filter,
                        "multiplierDecimal",
                        "PERCENT_PRICE.multiplierDecimal",
                    )?,
                });
            }
            "MIN_NOTIONAL" => {
                rules.min_notional =
                    Some(decimal_field(&filter, "notional", "MIN_NOTIONAL.notional")?);
            }
            "MAX_NUM_ORDERS" => {
                rules.max_orders = Some(integer_field(&filter, "limit", "MAX_NUM_ORDERS.limit")?);
            }
            "MAX_NUM_ALGO_ORDERS" => {
                rules.max_algo_orders = Some(integer_field(
                    &filter,
                    "limit",
                    "MAX_NUM_ALGO_ORDERS.limit",
                )?);
            }
            other => rules.unknown_filters.push(other.to_owned()),
        }
    }
    Ok(rules)
}

fn parse_lot_size(value: &Value, prefix: &'static str) -> Result<LotSizeFilter, BinanceError> {
    Ok(LotSizeFilter {
        min_quantity: decimal_field(value, "minQty", prefix)?,
        max_quantity: decimal_field(value, "maxQty", prefix)?,
        step_size: decimal_field(value, "stepSize", prefix)?,
    })
}

fn parse_kline_row(
    index: usize,
    row: &[Value],
    symbol: Symbol,
    interval: Interval,
    observed_at: DateTime<Utc>,
) -> Result<Kline, BinanceError> {
    if row.len() < 11 {
        return Err(BinanceError::InvalidResponse(format!(
            "kline row {index} has {} fields; expected at least 11",
            row.len()
        )));
    }
    let open_time = integer_value(&row[0], "kline.openTime")?;
    let close_time = integer_value(&row[6], "kline.closeTime")?;
    let close_time = Kline::timestamp(close_time)?;
    Ok(Kline {
        symbol,
        interval,
        open_time: Kline::timestamp(open_time)?,
        close_time,
        open: Price::new(decimal_value(&row[1], "kline.open")?)?,
        high: Price::new(decimal_value(&row[2], "kline.high")?)?,
        low: Price::new(decimal_value(&row[3], "kline.low")?)?,
        close: Price::new(decimal_value(&row[4], "kline.close")?)?,
        volume: decimal_value(&row[5], "kline.volume")?,
        quote_volume: decimal_value(&row[7], "kline.quoteVolume")?,
        trade_count: integer_value(&row[8], "kline.tradeCount")?,
        taker_buy_volume: decimal_value(&row[9], "kline.takerBuyVolume")?,
        taker_buy_quote_volume: decimal_value(&row[10], "kline.takerBuyQuoteVolume")?,
        is_closed: close_time < observed_at,
    })
}

fn decimal_field(value: &Value, key: &str, field: &'static str) -> Result<Decimal, BinanceError> {
    let raw = value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| BinanceError::InvalidResponse(format!("{field} is missing")))?;
    Decimal::from_str(raw).map_err(|_| BinanceError::InvalidDecimal {
        field,
        value: raw.to_owned(),
    })
}

fn integer_field<T>(value: &Value, key: &str, field: &'static str) -> Result<T, BinanceError>
where
    T: TryFrom<u64>,
{
    let raw = value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| BinanceError::InvalidResponse(format!("{field} is missing")))?;
    T::try_from(raw).map_err(|_| BinanceError::InvalidResponse(format!("{field} is out of range")))
}

fn decimal_value(value: &Value, field: &'static str) -> Result<Decimal, BinanceError> {
    let raw = value
        .as_str()
        .ok_or_else(|| BinanceError::InvalidResponse(format!("{field} must be text")))?;
    Decimal::from_str(raw).map_err(|_| BinanceError::InvalidDecimal {
        field,
        value: raw.to_owned(),
    })
}

fn integer_value<T>(value: &Value, field: &'static str) -> Result<T, BinanceError>
where
    T: TryFrom<i64>,
{
    let raw = value
        .as_i64()
        .ok_or_else(|| BinanceError::InvalidResponse(format!("{field} must be an integer")))?;
    T::try_from(raw).map_err(|_| BinanceError::InvalidResponse(format!("{field} is out of range")))
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::ContractStatus;

    const EXCHANGE_INFO_FIXTURE: &str = r#"{
      "serverTime": 1720000000000,
      "symbols": [{
        "symbol": "BTCUSDT",
        "contractType": "PERPETUAL",
        "status": "TRADING",
        "baseAsset": "BTC",
        "quoteAsset": "USDT",
        "marginAsset": "USDT",
        "onboardDate": 1569398400000,
        "filters": [
          {"filterType":"PRICE_FILTER","minPrice":"0.10","maxPrice":"1000000","tickSize":"0.10"},
          {"filterType":"LOT_SIZE","minQty":"0.001","maxQty":"1000","stepSize":"0.001"},
          {"filterType":"MARKET_LOT_SIZE","minQty":"0.001","maxQty":"100","stepSize":"0.001"},
          {"filterType":"PERCENT_PRICE","multiplierUp":"1.1500","multiplierDown":"0.8500","multiplierDecimal":4},
          {"filterType":"MIN_NOTIONAL","notional":"5"},
          {"filterType":"MAX_NUM_ORDERS","limit":200},
          {"filterType":"FUTURE_FILTER","value":"retained"}
        ]
      }]
    }"#;

    #[test]
    fn parses_exchange_filters_without_losing_unknown_types() {
        let info = parse_exchange_info(
            EXCHANGE_INFO_FIXTURE,
            Utc.timestamp_millis_opt(1_720_000_000_100)
                .single()
                .expect("valid timestamp"),
        )
        .expect("parse exchange info");
        let contract = info
            .contract(&Symbol::new("BTCUSDT").expect("valid symbol"))
            .expect("contract exists");
        assert_eq!(contract.status, ContractStatus::Trading);
        assert_eq!(contract.rules.min_notional, Some(Decimal::from(5)));
        assert_eq!(
            contract.rules.unknown_filters,
            vec!["FUTURE_FILTER".to_owned()]
        );
        assert!(info.raw_event.verify_hash());
    }

    #[test]
    fn parses_kline_array() {
        let payload = r#"[[1720000000000,"100.0","110.0","90.0","105.0","12.5",1720000899999,"1300.0",42,"7.5","780.0","0"]]"#;
        let rows = parse_klines(
            payload,
            Symbol::new("BTCUSDT").expect("valid symbol"),
            Interval::M15,
            Utc.timestamp_millis_opt(1_720_001_000_000)
                .single()
                .expect("valid timestamp"),
        )
        .expect("parse klines");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].trade_count, 42);
        assert_eq!(rows[0].close.value(), Decimal::from(105));
        assert!(rows[0].is_closed);
    }
}
