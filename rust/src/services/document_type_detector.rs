//! Document Type Detector
//!
//! Automatically detects document types based on file name, mime type,
//! and optional content analysis.

/// Document type detector
pub struct DocumentTypeDetector;

impl DocumentTypeDetector {
    /// Detect document type based on mime type and content
    pub async fn detect_type(
        mime_type: &str,
        file_name: &str,
        _file_bytes: &[u8], // For future OCR/AI detection
    ) -> Option<String> {
        // Simple rules based on filename patterns
        let name_lower = file_name.to_lowercase();

        // Identity documents
        if name_lower.contains("passport") {
            return Some("PASSPORT".to_string());
        } else if name_lower.contains("license")
            || name_lower.contains("driving")
            || name_lower.contains("driver")
        {
            return Some("DRIVERS_LICENSE".to_string());
        }

        // Financial documents
        if name_lower.contains("bank") || name_lower.contains("statement") {
            return Some("BANK_STATEMENT".to_string());
        } else if name_lower.contains("payslip") || name_lower.contains("pay_slip") {
            return Some("PAYSLIP".to_string());
        }

        // Proof of address
        if name_lower.contains("utility") || name_lower.contains("bill") {
            return Some("UTILITY_BILL".to_string());
        } else if name_lower.contains("council_tax") || name_lower.contains("rates") {
            return Some("COUNCIL_TAX_BILL".to_string());
        }

        // Employment documents
        if name_lower.contains("employment") || name_lower.contains("employer") {
            return Some("EMPLOYMENT_LETTER".to_string());
        } else if name_lower.contains("reference") {
            return Some("REFERENCE_LETTER".to_string());
        }

        // Corporate documents
        if name_lower.contains("articles") || name_lower.contains("incorporation") {
            return Some("ARTICLES_OF_INCORPORATION".to_string());
        } else if name_lower.contains("certificate") && name_lower.contains("incorporation") {
            return Some("CERTIFICATE_OF_INCORPORATION".to_string());
        } else if name_lower.contains("memorandum") {
            return Some("MEMORANDUM_OF_ASSOCIATION".to_string());
        }

        // Tax documents
        if name_lower.contains("tax")
            && (name_lower.contains("return") || name_lower.contains("assessment"))
        {
            return Some("TAX_RETURN".to_string());
        }

        // Default based on mime type
        match mime_type {
            "application/pdf" => Some("GENERIC_PDF".to_string()),
            "image/jpeg" | "image/jpg" | "image/png" => Some("GENERIC_IMAGE".to_string()),
            _ => None,
        }
    }

    /// Detect type with confidence score (future enhancement)
    pub async fn detect_type_with_confidence(
        mime_type: &str,
        file_name: &str,
        file_bytes: &[u8],
    ) -> Option<(String, f64)> {
        let detected = Self::detect_type(mime_type, file_name, file_bytes).await?;

        // Calculate confidence based on multiple factors
        let name_lower = file_name.to_lowercase();
        let confidence = if detected == "PASSPORT" && name_lower.contains("passport") {
            0.95
        } else if detected == "BANK_STATEMENT"
            && (name_lower.contains("bank") || name_lower.contains("statement"))
        {
            0.90
        } else if detected.starts_with("GENERIC_") {
            0.60 // Lower confidence for generic types
        } else {
            0.75 // Medium confidence for pattern matches
        };

        Some((detected, confidence))
    }
}
