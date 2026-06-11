pub(crate) trait ShellCapability: Sync {
    fn zsh_function_name(&self) -> Option<&'static str>;
}
