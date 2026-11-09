//! Port of `test/type.test.ts`.

use vega_lite::type_::*;

#[test]
fn get_full_name_should_return_correct_lowercase_full_type_names() {
    for t in ["q", "Q", "quantitative", "QUANTITATIVE"] {
        assert_eq!(get_full_name(t), Some(Type::Quantitative));
    }
    for t in ["t", "T", "temporal", "TEMPORAL"] {
        assert_eq!(get_full_name(t), Some(Type::Temporal));
    }
    for t in ["o", "O", "ordinal", "ORDINAL"] {
        assert_eq!(get_full_name(t), Some(Type::Ordinal));
    }
    for t in ["n", "N", "nominal", "NOMINAL"] {
        assert_eq!(get_full_name(t), Some(Type::Nominal));
    }
    for t in ["geojson", "GEOJSON"] {
        assert_eq!(get_full_name(t), Some(Type::Geojson));
    }
}

#[test]
fn get_full_name_should_return_undefined_for_invalid_type() {
    assert_eq!(get_full_name("haha"), None);
}
