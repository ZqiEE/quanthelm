use crate::BinanceError;

/// New high-frequency public WebSocket routing root.
pub const MAINNET_PUBLIC_WS_BASE: &str = "wss://fstream.binance.com/public";
/// New regular market-data WebSocket routing root.
pub const MAINNET_MARKET_WS_BASE: &str = "wss://fstream.binance.com/market";

/// WebSocket routing category introduced by Binance's classified URL architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketCategory {
    /// High-frequency public data such as trades or depth.
    Public,
    /// Regular market data such as mark-price streams.
    Market,
}

impl WebSocketCategory {
    const fn base_url(self) -> &'static str {
        match self {
            Self::Public => MAINNET_PUBLIC_WS_BASE,
            Self::Market => MAINNET_MARKET_WS_BASE,
        }
    }
}

/// Builds a classified single-stream URL without opening a connection.
pub fn single_stream_url(
    category: WebSocketCategory,
    stream: &str,
) -> Result<String, BinanceError> {
    validate_stream_name(stream)?;
    Ok(format!("{}/ws/{stream}", category.base_url()))
}

/// Builds a classified combined-stream URL without opening a connection.
pub fn combined_stream_url(
    category: WebSocketCategory,
    streams: &[String],
) -> Result<String, BinanceError> {
    if streams.is_empty() {
        return Err(BinanceError::InvalidRequest(
            "combined stream list must not be empty".to_owned(),
        ));
    }
    for stream in streams {
        validate_stream_name(stream)?;
    }
    Ok(format!(
        "{}/stream?streams={}",
        category.base_url(),
        streams.join("/")
    ))
}

fn validate_stream_name(stream: &str) -> Result<(), BinanceError> {
    let valid = !stream.is_empty()
        && stream.len() <= 256
        && stream.bytes().all(|byte| {
            byte.is_ascii_alphabetic()
                || byte.is_ascii_digit()
                || matches!(byte, b'@' | b'_' | b'-' | b'.')
        });
    if valid {
        Ok(())
    } else {
        Err(BinanceError::InvalidRequest(format!(
            "invalid WebSocket stream name: {stream}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classified_websocket_urls_are_built_safely() {
        assert_eq!(
            single_stream_url(WebSocketCategory::Market, "btcusdt@markPrice")
                .expect("valid stream"),
            "wss://fstream.binance.com/market/ws/btcusdt@markPrice"
        );
        assert!(single_stream_url(WebSocketCategory::Public, "../bad").is_err());
    }
}
