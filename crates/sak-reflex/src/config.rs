pub struct ReflexConfig {
    pub endpoint: String,
    pub token: String,
}

impl ReflexConfig {
    pub fn from_env() -> Self {
        Self {
            endpoint: std::env::var("YELLOWSTONE_ENDPOINT")
                .unwrap_or_else(|_| "https://sol-devnet-yellowstone-grpc.rpcfast.com:443".into()),
            token: std::env::var("YELLOWSTONE_TOKEN").unwrap_or_default(),
        }
    }

    pub fn devnet() -> Self {
        Self {
            endpoint: "https://sol-devnet-yellowstone-grpc.rpcfast.com:443".into(),
            token: std::env::var("YELLOWSTONE_TOKEN").unwrap_or_default(),
        }
    }

    pub fn custom(endpoint: &str, token: &str) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            token: token.to_string(),
        }
    }
}
