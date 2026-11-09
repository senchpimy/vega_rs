//! Port of `test/normalize/selectioncompat.test.ts`.

use serde_json::{json, Value};
use vega_lite::normalize::base::NormalizerParams;
use vega_lite::normalize::normalize;
use vega_lite::normalize::selectioncompat::map_unit;

fn unit() -> Value {
    json!({
        "data": {"url": "data/cars.json"},
        "mark": "point",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"}
        }
    })
}

fn norm_params() -> NormalizerParams {
    NormalizerParams::with_config(json!({}))
}

fn assert_array_containing(actual: &Value, expected: &[Value]) {
    let array = actual.as_array().expect("expected an array");
    for item in expected {
        assert!(
            array.contains(item),
            "expected array to contain {item}, got {actual}"
        );
    }
}

#[test]
fn should_normalize_single_selections() {
    let mut spec = unit();
    spec["selection"] = json!({
        "CylYr": {
            "type": "single",
            "fields": ["Cylinders", "Year"],
            "init": {"Cylinders": 4, "Year": 1977},
            "bind": {"input": "range", "step": 1},
            "nearest": true
        }
    });

    let normed_unit = map_unit(&spec, &norm_params()).unwrap();
    assert!(normed_unit.get("selection").is_none());
    let params = normed_unit["params"].as_array().unwrap();
    assert_eq!(params.len(), 1);
    assert_eq!(params[0]["name"], json!("CylYr"));
    assert_eq!(params[0]["value"], json!({"Cylinders": 4, "Year": 1977}));
    assert_eq!(params[0]["bind"], json!({"input": "range", "step": 1}));
    assert_eq!(
        params[0]["select"],
        json!({
            "type": "point",
            "fields": ["Cylinders", "Year"],
            "toggle": false,
            "nearest": true
        })
    );
}

#[test]
fn should_normalize_multi_selections() {
    let mut spec = unit();
    spec["selection"] = json!({
        "Org": {
            "type": "multi",
            "fields": ["Origin"],
            "init": {"Origin": "Japan"},
            "bind": "legend"
        }
    });

    let normed_unit = map_unit(&spec, &norm_params()).unwrap();
    assert!(normed_unit.get("selection").is_none());
    let params = normed_unit["params"].as_array().unwrap();
    assert_eq!(params.len(), 1);
    assert_eq!(params[0]["name"], json!("Org"));
    assert_eq!(params[0]["value"], json!({"Origin": "Japan"}));
    assert_eq!(params[0]["bind"], json!("legend"));
    assert_eq!(
        params[0]["select"],
        json!({"type": "point", "fields": ["Origin"]})
    );
}

#[test]
fn should_normalize_interval_selections() {
    let mut spec = unit();
    spec["selection"] = json!({
        "brush": {"type": "interval", "init": {"x": [55, 160], "y": [13, 37]}},
        "grid": {"type": "interval", "bind": "scales"}
    });

    let normed_unit = map_unit(&spec, &norm_params()).unwrap();
    assert!(normed_unit.get("selection").is_none());
    let params = normed_unit["params"].as_array().unwrap();
    assert_eq!(params.len(), 2);
    assert_eq!(params[0]["name"], json!("brush"));
    assert_eq!(params[1]["name"], json!("grid"));
    assert_eq!(params[0]["value"], json!({"x": [55, 160], "y": [13, 37]}));
    assert_eq!(params[1]["bind"], json!("scales"));
}

#[test]
fn should_normalize_selection_predicates() {
    let spec = json!({
        "data": {"url": "data/cars.json"},
        "transform": [
            {"filter": {"selection": "foo"}},
            {"filter": {"selection": {"and": ["foo", "bar"]}}},
            {"filter": {"or": [{"selection": {"not": "foo"}}, "false"]}}
        ],
        "selection": {
            "foo": {"type": "single", "empty": "all"},
            "bar": {"type": "multi", "empty": "none"},
            "brush": {"type": "interval"}
        },
        "mark": "line",
        "encoding": {
            "x": {
                "field": "Horsepower",
                "type": "quantitative",
                "condition": {"selection": "bar", "value": 5}
            },
            "y": {
                "value": 10,
                "condition": {
                    "selection": {"or": ["foo", "brush"]},
                    "field": "Miles_per_Gallon",
                    "type": "quantitative"
                }
            },
            "color": {
                "condition": {
                    "test": {"and": [{"selection": {"not": "foo"}}, "true"]},
                    "field": "Cylinders",
                    "type": "ordinal"
                },
                "value": "grey"
            },
            "strokeWidth": {
                "condition": [
                    {
                        "test": {"and": [{"selection": "foo"}, "length(data(\"foo_store\"))"]},
                        "value": 2
                    },
                    {"selection": "bar", "value": 1}
                ],
                "value": 0
            }
        }
    });

    let normalized = normalize(&spec, None).unwrap();
    assert_array_containing(
        &normalized["transform"],
        &[
            json!({"filter": {"param": "foo", "empty": true}}),
            json!({"filter": {"and": [{"param": "foo", "empty": true}, {"param": "bar", "empty": false}]}}),
            json!({"filter": {"or": [{"not": {"param": "foo", "empty": true}}, "false"]}}),
        ],
    );
    assert_eq!(
        normalized["encoding"]["x"]["condition"]["test"],
        json!({"param": "bar", "empty": false})
    );
    assert_eq!(
        normalized["encoding"]["y"]["condition"]["test"],
        json!({
            "or": [
                {"param": "foo", "empty": true},
                {"param": "brush", "empty": true}
            ]
        })
    );
    assert_eq!(
        normalized["encoding"]["color"]["condition"]["test"],
        json!({"and": [{"not": {"param": "foo", "empty": true}}, "true"]})
    );
    assert_eq!(
        normalized["encoding"]["strokeWidth"]["condition"],
        json!([
            {
                "value": 2,
                "test": {"and": [{"param": "foo", "empty": true}, "length(data(\"foo_store\"))"]}
            },
            {
                "value": 1,
                "test": {"param": "bar", "empty": false}
            }
        ])
    );

    assert_eq!(normalized["encoding"]["x"]["field"], json!("Horsepower"));
    assert_eq!(normalized["encoding"]["x"]["type"], json!("quantitative"));
    assert_eq!(normalized["encoding"]["x"]["condition"]["value"], json!(5));
    assert_eq!(normalized["encoding"]["y"]["value"], json!(10));
    assert_eq!(
        normalized["encoding"]["y"]["condition"]["field"],
        json!("Miles_per_Gallon")
    );
    assert_eq!(
        normalized["encoding"]["y"]["condition"]["type"],
        json!("quantitative")
    );
    assert_eq!(normalized["encoding"]["color"]["value"], json!("grey"));
    assert_eq!(
        normalized["encoding"]["color"]["condition"]["field"],
        json!("Cylinders")
    );
    assert_eq!(
        normalized["encoding"]["color"]["condition"]["type"],
        json!("ordinal")
    );
}

#[test]
fn should_normalize_bin_extents() {
    let spec = json!({
        "data": {"url": "data/cars.json"},
        "transform": [
            {"bin": true, "field": "Horsepower", "as": "bin_HP"},
            {"bin": {"extent": {"selection": "foo"}, "nice": true}, "field": "Miles_per_Gallon", "as": "bin_MPG"},
            {"bin": {"extent": {"selection": "bar", "field": "Accl"}, "nice": false}, "field": "Acceleration", "as": "bin_Accl"}
        ],
        "selection": {
            "foo": {"type": "single"},
            "bar": {"type": "multi"},
            "brush": {"type": "interval"}
        },
        "mark": "line",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative", "bin": true},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative", "bin": {"extent": {"selection": "foo"}, "nice": true}},
            "color": {"field": "Acceleration", "type": "quantitative", "bin": {"extent": {"selection": "foo", "encoding": "x"}, "nice": false}},
            "size": {
                "value": 50,
                "condition": {"selection": "bar", "field": "Displacement", "type": "quantitative", "bin": true}
            },
            "opacity": {
                "value": 1,
                "condition": {
                    "selection": "foo",
                    "field": "Weight_in_lbs",
                    "type": "quantitative",
                    "bin": {"extent": {"selection": "brush", "field": "Lbs"}, "nice": true}
                }
            }
        }
    });

    let normalized = normalize(&spec, None).unwrap();
    assert_array_containing(
        &normalized["transform"],
        &[
            json!({"bin": true, "field": "Horsepower", "as": "bin_HP"}),
            json!({"bin": {"extent": {"param": "foo"}, "nice": true}, "field": "Miles_per_Gallon", "as": "bin_MPG"}),
            json!({"bin": {"extent": {"param": "bar", "field": "Accl"}, "nice": false}, "field": "Acceleration", "as": "bin_Accl"}),
        ],
    );

    assert_eq!(normalized["encoding"]["x"]["bin"], json!(true));
    assert_eq!(
        normalized["encoding"]["y"]["bin"],
        json!({"extent": {"param": "foo"}, "nice": true})
    );
    assert_eq!(
        normalized["encoding"]["color"]["bin"],
        json!({"extent": {"param": "foo", "encoding": "x"}, "nice": false})
    );
    assert_eq!(normalized["encoding"]["size"]["condition"]["bin"], json!(true));
    assert_eq!(
        normalized["encoding"]["opacity"]["condition"]["bin"],
        json!({"extent": {"param": "brush", "field": "Lbs"}, "nice": true})
    );
}

#[test]
fn should_normalize_scale_domains() {
    let spec = json!({
        "data": {"url": "data/cars.json"},
        "selection": {
            "foo": {"type": "single"},
            "bar": {"type": "multi"},
            "brush": {"type": "interval"}
        },
        "mark": "line",
        "encoding": {
            "x": {
                "field": "Horsepower",
                "type": "quantitative",
                "scale": {"domain": {"selection": "brush"}, "reverse": true}
            },
            "y": {
                "field": "Miles_per_Gallon",
                "type": "quantitative",
                "scale": {"domain": {"selection": "bar", "field": "MPG"}, "round": true}
            },
            "size": {
                "value": 50,
                "condition": {
                    "selection": "bar",
                    "field": "Displacement",
                    "type": "quantitative",
                    "scale": {"domain": {"selection": "foo"}, "reverse": true}
                }
            },
            "opacity": {
                "value": 1,
                "condition": {
                    "selection": "foo",
                    "field": "Weight_in_lbs",
                    "type": "quantitative",
                    "scale": {"domain": {"selection": "foo", "encoding": "y"}, "round": true}
                }
            }
        }
    });

    let normalized = normalize(&spec, None).unwrap();
    assert_eq!(
        normalized["encoding"]["x"]["scale"],
        json!({"domain": {"param": "brush"}, "reverse": true})
    );
    assert_eq!(
        normalized["encoding"]["y"]["scale"],
        json!({"domain": {"param": "bar", "field": "MPG"}, "round": true})
    );
    assert_eq!(
        normalized["encoding"]["size"]["condition"]["scale"],
        json!({"domain": {"param": "foo"}, "reverse": true})
    );
    assert_eq!(
        normalized["encoding"]["opacity"]["condition"]["scale"],
        json!({"domain": {"param": "foo", "encoding": "y"}, "round": true})
    );
}

#[test]
fn should_normalize_lookups() {
    let spec = json!({
        "data": {"url": "data/cars.json"},
        "transform": [
            {
                "lookup": "person",
                "from": {
                    "data": {"url": "data/lookup_people.csv"},
                    "key": "name",
                    "fields": ["age", "height"]
                }
            },
            {
                "lookup": "symbol",
                "from": {"selection": "index", "key": "symbol"},
                "default": 5
            }
        ],
        "selection": {
            "foo": {"type": "single"},
            "bar": {"type": "multi"},
            "brush": {"type": "interval"}
        },
        "mark": "line",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"}
        }
    });

    let normalized = normalize(&spec, None).unwrap();
    assert_array_containing(
        &normalized["transform"],
        &[
            json!({
                "lookup": "person",
                "from": {
                    "data": {"url": "data/lookup_people.csv"},
                    "key": "name",
                    "fields": ["age", "height"]
                }
            }),
            json!({
                "lookup": "symbol",
                "from": {"param": "index", "key": "symbol"},
                "default": 5
            }),
        ],
    );
}

#[test]
fn should_be_the_first_normalizer_run() {
    let spec = json!({
        "data": {"url": "data/cars.json"},
        "selection": {
            "brush": {"type": "interval", "init": {"x": [55, 160], "y": [13, 37]}}
        },
        "mark": {"type": "line", "point": true},
        "encoding": {
            "row": {"field": "Origin", "type": "nominal"},
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"},
            "color": {
                "condition": {"selection": "brush", "field": "Cylinders", "type": "ordinal"},
                "value": "grey"
            }
        }
    });

    let normalized = normalize(&spec, None).unwrap();
    assert!(normalized["spec"]["layer"][0].get("params").is_some());
    assert_eq!(normalized["spec"]["layer"][0]["params"][0]["name"], json!("brush"));
}

#[test]
fn should_normalize_multi_views() {
    let spec = json!({
        "data": {"url": "data/stocks.csv"},
        "encoding": {
            "color": {
                "condition": {"selection": "hover", "field": "symbol", "type": "nominal"},
                "value": "grey"
            }
        },
        "layer": [
            {
                "encoding": {
                    "x": {"field": "date", "type": "temporal", "title": "date"},
                    "y": {"field": "price", "type": "quantitative", "title": "price"}
                },
                "layer": [
                    {
                        "selection": {
                            "hover": {
                                "type": "single",
                                "on": "pointerover",
                                "empty": "all",
                                "fields": ["symbol"],
                                "init": {"symbol": "AAPL"}
                            }
                        },
                        "mark": {"type": "line", "strokeWidth": 8, "stroke": "transparent"}
                    },
                    {"mark": "line"}
                ]
            },
            {
                "mark": {"type": "circle"},
                "encoding": {
                    "x": {"aggregate": "max", "field": "date", "type": "temporal"},
                    "y": {"aggregate": {"argmax": "date"}, "field": "price", "type": "quantitative"}
                }
            }
        ]
    });

    assert_eq!(
        normalize(&spec, None).unwrap(),
        json!({
            "data": {"url": "data/stocks.csv"},
            "layer": [
                {
                    "layer": [
                        {
                            "mark": {"type": "line", "strokeWidth": 8, "stroke": "transparent"},
                            "params": [
                                {
                                    "name": "hover",
                                    "value": {"symbol": "AAPL"},
                                    "select": {
                                        "type": "point",
                                        "on": "pointerover",
                                        "fields": ["symbol"],
                                        "toggle": false
                                    }
                                }
                            ],
                            "encoding": {
                                "color": {
                                    "condition": {
                                        "field": "symbol",
                                        "type": "nominal",
                                        "test": {"param": "hover", "empty": true}
                                    },
                                    "value": "grey"
                                },
                                "x": {"field": "date", "type": "temporal", "title": "date"},
                                "y": {"field": "price", "type": "quantitative", "title": "price"}
                            }
                        },
                        {
                            "mark": "line",
                            "encoding": {
                                "color": {
                                    "condition": {
                                        "field": "symbol",
                                        "type": "nominal",
                                        "test": {"param": "hover", "empty": true}
                                    },
                                    "value": "grey"
                                },
                                "x": {"field": "date", "type": "temporal", "title": "date"},
                                "y": {"field": "price", "type": "quantitative", "title": "price"}
                            }
                        }
                    ]
                },
                {
                    "mark": {"type": "circle"},
                    "encoding": {
                        "color": {
                            "condition": {
                                "field": "symbol",
                                "type": "nominal",
                                "test": {"param": "hover", "empty": true}
                            },
                            "value": "grey"
                        },
                        "x": {"aggregate": "max", "field": "date", "type": "temporal"},
                        "y": {
                            "aggregate": {"argmax": "date"},
                            "field": "price",
                            "type": "quantitative"
                        }
                    }
                }
            ]
        })
    );
}
