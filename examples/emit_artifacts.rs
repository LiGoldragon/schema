use schema_next::{MacroLibrary, MacroLibraryArtifact, SchemaEngine, SchemaIdentity};

fn main() {
    let macro_library = MacroLibrary::from_source(include_str!("../schemas/builtin-macros.schema"))
        .expect("builtin macro source lowers");
    println!("=== builtin-macros.macro-library ===");
    println!(
        "{}",
        MacroLibraryArtifact::new(macro_library).to_nota_source()
    );

    let core_source = include_str!("../schemas/core.schema");
    let core_asschema = SchemaEngine::default()
        .lower_source(
            core_source,
            SchemaIdentity::new("schema-next:core", "0.1.0"),
        )
        .expect("core schema lowers");
    println!("=== core.asschema ===");
    println!("{}", core_asschema.to_nota());
}
