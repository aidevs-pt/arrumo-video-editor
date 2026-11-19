use std::{env, error::Error, io, path::Path};

use arrumo_workspaces::WorkspaceStore;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    match arguments.as_slice() {
        [database, command] if command == "list" => {
            let workspaces = WorkspaceStore::open(Path::new(database))?.list()?;
            println!("{}", serde_json::to_string(&workspaces)?);
        }
        [database, command, name] if command == "create" => {
            let name = name.to_str().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Name must be valid UTF-8")
            })?;
            let workspace = WorkspaceStore::open(Path::new(database))?.create(name)?;
            println!("{}", serde_json::to_string(&workspace)?);
        }
        _ => {
            return Err("Usage: arrumo-workspaces <database> list | create <name>".into());
        }
    }
    Ok(())
}
