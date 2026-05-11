pub struct ReflexConfig {
    pub endpoint: String,
    pub token: String,
}

impl ReflexConfig {
    /// Reads `GEYSER_ENDPOINT` or `YELLOWSTONE_ENDPOINT`, then default devnet gRPC.
    pub fn from_env() -> Self {
        Self {
            endpoint: std::env::var("GEYSER_ENDPOINT")
                .or_else(|_| std::env::var("YELLOWSTONE_ENDPOINT"))
                .unwrap_or_else(|_| "https://sol-devnet-yellowstone-grpc.rpcfast.com:443".into()),
            token: std::env::var("HELIUS_API_KEY")
                .or_else(|_| std::env::var("YELLOWSTONE_TOKEN"))
                .unwrap_or_default(),
        }
    }

    /// Default devnet endpoint; token from `HELIUS_API_KEY` or `YELLOWSTONE_TOKEN`.
    pub fn devnet() -> Self {
        Self {
            endpoint: "https://sol-devnet-yellowstone-grpc.rpcfast.com:443".into(),
            token: std::env::var("HELIUS_API_KEY")
                .or_else(|_| std::env::var("YELLOWSTONE_TOKEN"))
                .unwrap_or_default(),
        }
    }

    /// Merge explicit kernel-style options with env fallbacks (used by `sak-bin`).
    pub fn from_kernel_options(
        geyser_endpoint: Option<String>,
        helius_api_key: Option<String>,
    ) -> Self {
        let endpoint = geyser_endpoint
            .or_else(|| std::env::var("GEYSER_ENDPOINT").ok())
            .or_else(|| std::env::var("YELLOWSTONE_ENDPOINT").ok())
            .unwrap_or_else(|| "https://sol-devnet-yellowstone-grpc.rpcfast.com:443".into());
        let token = helius_api_key
            .or_else(|| std::env::var("HELIUS_API_KEY").ok())
            .or_else(|| std::env::var("YELLOWSTONE_TOKEN").ok())
            .unwrap_or_default();
        Self { endpoint, token }
    }

    pub fn custom(endpoint: &str, token: &str) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            token: token.to_string(),
        }
    }
}
