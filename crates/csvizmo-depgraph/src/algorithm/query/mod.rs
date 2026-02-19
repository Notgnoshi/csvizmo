pub mod edges;
pub mod metrics;
pub mod nodes;

#[derive(Debug, Default, Clone, Copy, clap::ValueEnum)]
pub enum OutputFields {
    Id,
    #[default]
    Label,
}

impl std::fmt::Display for OutputFields {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use clap::ValueEnum;

        f.write_str(self.to_possible_value().unwrap().get_name())
    }
}
