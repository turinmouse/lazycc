pub(crate) trait EnvCapability: Sync {
    fn base_url_env(&self) -> &'static str;
    fn api_key_env(&self) -> &'static str;
    fn model_env(&self) -> Option<&'static str>;
    fn all_envs(&self) -> &'static [&'static str];
}
