//! Generated snapshot inventory; regenerate entries when regions change.
pub(crate) fn region(name: &str) -> bool {
    matches!(
        name,
        "consultation"
            | "crystal"
            | "events"
            | "icemule"
            | "instanced"
            | "landing"
            | "mist-harbor"
            | "open-sea"
            | "rivers-rest"
            | "solhaven"
            | "taillistim"
            | "tavaalor"
            | "teras"
            | "transport"
            | "unassigned-region"
            | "zul-logoth"
    )
}

pub(crate) fn get(path: &str) -> Option<&'static str> {
    Some(match path {
        "consultation/data.json" => include_str!("../atlas-data/consultation/data.json"),
        "consultation/region-data.json" => {
            include_str!("../atlas-data/consultation/region-data.json")
        }
        "corpus/manifest.json" => include_str!("../atlas-data/corpus/manifest.json"),
        "corpus/search.json" => include_str!("../atlas-data/corpus/search.json"),
        "corpus/world.json" => include_str!("../atlas-data/corpus/world.json"),
        "crystal/data.json" => include_str!("../atlas-data/crystal/data.json"),
        "crystal/region-data.json" => include_str!("../atlas-data/crystal/region-data.json"),
        "events/data.json" => include_str!("../atlas-data/events/data.json"),
        "events/region-data.json" => include_str!("../atlas-data/events/region-data.json"),
        "icemule/data.json" => include_str!("../atlas-data/icemule/data.json"),
        "icemule/region-data.json" => include_str!("../atlas-data/icemule/region-data.json"),
        "instanced/data.json" => include_str!("../atlas-data/instanced/data.json"),
        "instanced/region-data.json" => {
            include_str!("../atlas-data/instanced/region-data.json")
        }
        "landing/data.json" => include_str!("../atlas-data/landing/data.json"),
        "landing/region-data.json" => include_str!("../atlas-data/landing/region-data.json"),
        "mist-harbor/data.json" => include_str!("../atlas-data/mist-harbor/data.json"),
        "mist-harbor/region-data.json" => {
            include_str!("../atlas-data/mist-harbor/region-data.json")
        }
        "open-sea/data.json" => include_str!("../atlas-data/open-sea/data.json"),
        "open-sea/region-data.json" => include_str!("../atlas-data/open-sea/region-data.json"),
        "rivers-rest/data.json" => include_str!("../atlas-data/rivers-rest/data.json"),
        "rivers-rest/region-data.json" => {
            include_str!("../atlas-data/rivers-rest/region-data.json")
        }
        "solhaven/data.json" => include_str!("../atlas-data/solhaven/data.json"),
        "solhaven/region-data.json" => include_str!("../atlas-data/solhaven/region-data.json"),
        "taillistim/data.json" => include_str!("../atlas-data/taillistim/data.json"),
        "taillistim/region-data.json" => {
            include_str!("../atlas-data/taillistim/region-data.json")
        }
        "tavaalor/data.json" => include_str!("../atlas-data/tavaalor/data.json"),
        "tavaalor/region-data.json" => include_str!("../atlas-data/tavaalor/region-data.json"),
        "teras/data.json" => include_str!("../atlas-data/teras/data.json"),
        "teras/region-data.json" => include_str!("../atlas-data/teras/region-data.json"),
        "transport/data.json" => include_str!("../atlas-data/transport/data.json"),
        "transport/region-data.json" => {
            include_str!("../atlas-data/transport/region-data.json")
        }
        "unassigned-region/data.json" => {
            include_str!("../atlas-data/unassigned-region/data.json")
        }
        "unassigned-region/region-data.json" => {
            include_str!("../atlas-data/unassigned-region/region-data.json")
        }
        "zul-logoth/data.json" => include_str!("../atlas-data/zul-logoth/data.json"),
        "zul-logoth/region-data.json" => {
            include_str!("../atlas-data/zul-logoth/region-data.json")
        }
        _ => return None,
    })
}
