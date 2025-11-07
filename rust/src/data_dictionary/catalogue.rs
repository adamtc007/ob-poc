//! Attribute catalogue operations

use super::*;

pub struct AttributeCatalogue {
    dictionary: DataDictionary,
}

impl AttributeCatalogue {
    pub fn new(dictionary: DataDictionary) -> Self {
        AttributeCatalogue { dictionary }
    }

    pub fn search_by_semantic_similarity(
        &self,
        query_vector: &[f32],
        top_k: usize,
    ) -> Vec<(&String, f64)> {
        let mut results: Vec<(&String, f64)> = self
            .dictionary
            .attributes
            .iter()
            .filter_map(|(id, attr)| {
                attr.embedding
                    .as_ref()
                    .and_then(|emb| emb.vector.as_ref())
                    .map(|vec| {
                        let similarity = cosine_similarity(query_vector, vec);
                        (id, similarity)
                    })
            })
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results.truncate(top_k);
        results
    }

    pub fn find_related_attributes(&self, attr_id: &str) -> Vec<&AttributeDefinition> {
        if let Some(attr) = self.dictionary.get_attribute(attr_id) {
            let mut related = Vec::new();
            for concept in &attr.semantic.related_concepts {
                if let Some(related_attr) = self.dictionary.get_attribute(concept) {
                    related.push(related_attr);
                }
            }
            related
        } else {
            Vec::new()
        }
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if magnitude_a == 0.0 || magnitude_b == 0.0 {
        return 0.0;
    }

    (dot_product / (magnitude_a * magnitude_b)) as f64
}
