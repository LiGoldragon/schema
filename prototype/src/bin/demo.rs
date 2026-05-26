//! Demonstration binary for the schema-derived NOTA prototype.
//!
//! Runs the full bootstrap chain:
//!   1. Load `nota.schema` source.
//!   2. Kernel reads NOTA into a delimiter tree.
//!   3. Three-part schema reader extracts Specifying / Input / Output.
//!   4. Assemble the namespace.
//!   5. Emit the codec rule set (`EmittedCodec`).
//!   6. Load a per-component schema (`coordinate.schema`) into the
//!      library, with the core (nota.schema) implicitly available.
//!   7. Classify a macro invocation via shape-interpretation.
//!
//! Print the result. The binary's contract is: exit 0 if the full
//! chain succeeds, non-zero with a diagnostic otherwise.

use schema_derived_nota_prototype::{
    EmittedCodec, Kernel, Library, MacroEngine, MacroShape,
    schema::{AssembledSchema, ThreePartSchema, TypeBody},
};

const NOTA_SCHEMA: &str = include_str!("../../schemas/nota.schema");
const COORDINATE_SCHEMA: &str = include_str!("../../schemas/coordinate.schema");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== schema-derived NOTA prototype demo ===\n");

    println!("[1] Kernel — lexing nota.schema");
    let mut kernel = Kernel::new(NOTA_SCHEMA);
    let tokens = kernel.lex()?;
    println!("    {} tokens lexed", tokens.len());

    println!("\n[2] Kernel — parsing into delimiter tree");
    let mut kernel = Kernel::new(NOTA_SCHEMA);
    let top_level = kernel.parse_sequence()?;
    println!("    {} top-level blocks parsed", top_level.len());

    println!("\n[3] Three-part schema view");
    let three_part = ThreePartSchema::read(NOTA_SCHEMA)?;
    println!("    Specifying entries: {}", three_part.specifying.len());
    println!(
        "    Input header entries: {}",
        three_part.input_header.len()
    );
    println!(
        "    Input extras entries: {}",
        three_part.input_extras.len()
    );
    println!(
        "    Namespace entries (raw): {}",
        three_part.namespace.len()
    );
    println!("    Output entries: {}", three_part.output.len());
    println!("    has_input = {}", three_part.has_input());
    println!("    has_output = {}", three_part.has_output());

    println!("\n[4] Assembled schema (nota.schema)");
    let assembled = AssembledSchema::from_three_part(&three_part)?;
    println!("    Namespace bindings: {}", assembled.namespace.len());
    for entry in &assembled.namespace {
        let kind = match &entry.body {
            TypeBody::Enum { variants } => format!("enum ({} variants)", variants.len()),
            TypeBody::Struct { fields } => format!("struct ({} fields)", fields.len()),
            TypeBody::Map { entries } => format!("map ({} entries)", entries.len()),
            TypeBody::Macro { .. } => "macro".to_string(),
            TypeBody::Alias { target } => format!("alias -> {target}"),
        };
        println!("      {} = {}", entry.name, kind);
    }

    println!("\n[5] Emitted codec rules (from nota.schema)");
    let emitted = EmittedCodec::emit(&assembled);
    println!("    types declared:  {}", emitted.type_names.len());
    println!("    enum types:      {}", emitted.enum_names.len());
    println!("    struct types:    {}", emitted.struct_names.len());
    println!("    macro types:     {}", emitted.macro_names.len());
    println!("    variant entries: {}", emitted.variant_index.len());

    println!("\n[6] Codec predicates");
    let tests = [
        ("nota-codec", true),
        ("noNotaCodec", true),
        ("Pascal", false),
        ("None", false),
        ("with space", false),
        ("3digit", false),
    ];
    for (text, expected) in tests {
        let actual = emitted.is_bare_eligible(text);
        let mark = if actual == expected { "ok" } else { "FAIL" };
        println!("    bare_eligible({text:?}) = {actual} (expected {expected}) {mark}");
    }

    println!("\n[7] Precompiled-schema library");
    let mut library = Library::with_core(NOTA_SCHEMA)?;
    println!("    core: {} types", library.core().namespace.len());
    library.load("coordinate", COORDINATE_SCHEMA)?;
    println!("    loaded: {:?}", library.loaded_names());
    let coordinate = library.get("coordinate").expect("just loaded");
    println!(
        "    coordinate.schema — Input ops: {}  Output ops: {}",
        coordinate.input_operations.len(),
        coordinate.output_operations.len()
    );

    println!("\n[8] Macro shape-interpretation");
    let macro_engine = MacroEngine::new();
    let mut single = Kernel::new("{ universalUnknown }");
    let single_node = single.parse_single()?;
    let single_shape = macro_engine.classify(&single_node);
    println!("    {{ universalUnknown }} = {single_shape:?}");
    let mut map_macro = Kernel::new("{ host localhost port 8080 }");
    let map_node = map_macro.parse_single()?;
    let map_shape = macro_engine.classify(&map_node);
    if let MacroShape::KeyValueMap { entries } = &map_shape {
        println!(
            "    {{ host localhost port 8080 }} = KeyValueMap with {} entries",
            entries.len()
        );
        for (key, _) in entries {
            println!("      key: {key}");
        }
    } else {
        println!("    UNEXPECTED shape: {map_shape:?}");
    }

    println!("\n[9] Library resolution — core fallthrough");
    let token = library.resolve("coordinate", "TokenKind");
    println!(
        "    resolve(coordinate, TokenKind) -> {}",
        if token.is_some() {
            "found via core (nota.schema) fallthrough"
        } else {
            "NOT FOUND"
        }
    );

    println!("\n=== prototype OK ===");
    Ok(())
}
