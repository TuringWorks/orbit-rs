pub struct Transformer;
pub struct TransformerBuilder;

impl TransformerBuilder {
    pub fn build(self) -> crate::error::Result<Transformer> {
        Ok(Transformer)
    }
}
