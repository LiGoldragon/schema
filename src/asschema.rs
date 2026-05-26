use std::fmt;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Name(String);

impl Name {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn field_name(&self) -> String {
        let mut output = String::new();
        for (index, character) in self.0.chars().enumerate() {
            if character.is_ascii_uppercase() {
                if index > 0 {
                    output.push('_');
                }
                output.push(character.to_ascii_lowercase());
            } else if character == '-' {
                output.push('_');
            } else {
                output.push(character);
            }
        }
        output
    }
}

impl fmt::Display for Name {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Asschema {
    identity: super::SchemaIdentity,
    imports: Vec<ImportDeclaration>,
    surfaces: Vec<RootSurface>,
    namespace: Vec<TypeDeclaration>,
}

impl Asschema {
    pub(crate) fn new(
        identity: super::SchemaIdentity,
        imports: Vec<ImportDeclaration>,
        surfaces: Vec<RootSurface>,
        namespace: Vec<TypeDeclaration>,
    ) -> Self {
        Self {
            identity,
            imports,
            surfaces,
            namespace,
        }
    }

    pub fn identity(&self) -> &super::SchemaIdentity {
        &self.identity
    }

    pub fn imports(&self) -> &[ImportDeclaration] {
        &self.imports
    }

    pub fn surfaces(&self) -> &[RootSurface] {
        &self.surfaces
    }

    pub fn namespace(&self) -> &[TypeDeclaration] {
        &self.namespace
    }

    pub fn type_named(&self, name: &str) -> Option<&TypeDeclaration> {
        self.namespace
            .iter()
            .find(|declaration| declaration.name().as_str() == name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportDeclaration {
    pub local_name: Name,
    pub source: TypeReference,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootSurface {
    pub name: Name,
    pub variants: Vec<EnumVariant>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeDeclaration {
    Struct(StructDeclaration),
    Enum(EnumDeclaration),
    Newtype(StructDeclaration),
}

impl TypeDeclaration {
    pub fn name(&self) -> &Name {
        match self {
            Self::Struct(declaration) | Self::Newtype(declaration) => &declaration.name,
            Self::Enum(declaration) => &declaration.name,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructDeclaration {
    pub name: Name,
    pub fields: Vec<FieldDeclaration>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDeclaration {
    pub name: Name,
    pub reference: TypeReference,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumDeclaration {
    pub name: Name,
    pub variants: Vec<EnumVariant>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumVariant {
    pub name: Name,
    pub payload: Option<TypeReference>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeReference {
    pub name: Name,
}

impl TypeReference {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: Name::new(name),
        }
    }
}
