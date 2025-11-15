//! Attribute catalogue operations

use super::*;

#[cfg(feature = "database")]
pub(crate) struct AttributeCatalogue {
    dictionary: DataDictionary,
}

#[cfg(feature = "database")]
impl AttributeCatalogue {
    pub fn new(dictionary: DataDictionary) -> Self {
        AttributeCatalogue { dictionary }
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
