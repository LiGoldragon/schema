use nota_codec::{NotaValue, NotaValueKind};

use crate::{Error, Result, engine::NodeDefinitionPoint};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeDefinitionShape {
    ImportDirective,
    HeaderRoot,
    NamespaceValue(NamespaceValueShape),
    FeatureItem,
    UpgradeRule,
}

impl NodeDefinitionShape {
    pub fn recognize(point: NodeDefinitionPoint, value: &NotaValue) -> Result<Self> {
        match point {
            NodeDefinitionPoint::ImportMapValue => {
                expect_kind(point, value, &[NotaValueKind::Record])?;
                Ok(Self::ImportDirective)
            }
            NodeDefinitionPoint::HeaderRoot => {
                expect_kind(point, value, &[NotaValueKind::Record])?;
                Ok(Self::HeaderRoot)
            }
            NodeDefinitionPoint::NamespaceValue => {
                Ok(Self::NamespaceValue(NamespaceValueShape::recognize(value)?))
            }
            NodeDefinitionPoint::FeatureItem => {
                expect_kind(point, value, &[NotaValueKind::Record])?;
                Ok(Self::FeatureItem)
            }
            NodeDefinitionPoint::UpgradeRule => {
                expect_kind(point, value, &[NotaValueKind::Record])?;
                Ok(Self::UpgradeRule)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NamespaceValueShape {
    Enum,
    Record,
    Newtype,
    Alias,
}

impl NamespaceValueShape {
    pub fn recognize(value: &NotaValue) -> Result<Self> {
        match value.kind() {
            NotaValueKind::Sequence if is_newtype_sequence(value) => Ok(Self::Newtype),
            NotaValueKind::Sequence => Ok(Self::Record),
            NotaValueKind::Identifier => Ok(Self::Alias),
            NotaValueKind::Record => Ok(Self::Enum),
            _ => Err(unsupported_shape(
                NodeDefinitionPoint::NamespaceValue,
                value,
                "sequence, record, or identifier",
            )),
        }
    }
}

fn is_newtype_sequence(value: &NotaValue) -> bool {
    matches!(value.as_sequence(), Some([_]))
}

fn expect_kind(
    point: NodeDefinitionPoint,
    value: &NotaValue,
    allowed: &[NotaValueKind],
) -> Result<()> {
    if allowed.contains(&value.kind()) {
        return Ok(());
    }
    Err(unsupported_shape(
        point,
        value,
        &allowed
            .iter()
            .map(|kind| format!("{kind:?}"))
            .collect::<Vec<_>>()
            .join(" or "),
    ))
}

fn unsupported_shape(point: NodeDefinitionPoint, value: &NotaValue, expected: &str) -> Error {
    Error::InvalidSchemaText {
        context: "node definition shape",
        message: format!("{point:?} expected {expected}, got {:?}", value.kind()),
    }
}
