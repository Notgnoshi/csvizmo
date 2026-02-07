use clap::ValueEnum;

#[derive(Clone, Debug, ValueEnum)]
pub enum InputFormat {
    Dot,
    Mermaid,
    Tgf,
    Depfile,
    CargoMetadata,
    CargoTree,
    Tree,
    Pathlist,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum OutputFormat {
    Dot,
    Mermaid,
    Tgf,
    Depfile,
    Tree,
    Pathlist,
}
