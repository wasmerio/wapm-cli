#![cfg(test)]

use maplit::hashmap;

use super::prelude::*;

/*
#[test]
fn it_works() {
    let _t = set_test_dir_to_new_temp_dir();
    set_registry_to_dev().unwrap();
    init_manifest().unwrap();
    let manifest = get_manifest().unwrap();
    assert!(manifest.dependencies.is_none());

    {
        let result = add_dependencies(&["this-package-does-not-exist"]);
        assert!(result.is_err());
        let manifest = get_manifest().unwrap();
        assert!(manifest.dependencies.is_none());
    }
    {
        add_dependencies(&["mark2/python@0.0.4", "mark2/dog2"]).unwrap();
        let manifest = get_manifest().unwrap();
        assert_eq!(
            manifest.dependencies,
            Some(hashmap! {
                "mark2/python".to_string() => "0.0.4".to_string(),
                "mark2/dog2".to_string() => "0.0.13".to_string(),
            })
        );
    }
    {
        add_dependencies(&["lolcat@0.1.1"]).unwrap();
        let manifest_before = get_manifest().unwrap();
        remove_dependencies(&["lolcat"]).unwrap();
        let manifest_after = get_manifest().unwrap();

        assert_eq!(
            manifest_before.dependencies,
            Some(hashmap! {
                "mark2/python".to_string() => "0.0.4".to_string(),
                "mark2/dog2".to_string() => "0.0.13".to_string(),
                "lolcat".to_string() => "0.1.1".to_string(),
            })
        );
        assert_eq!(
            manifest_after.dependencies,
            Some(hashmap! {
                "mark2/python".to_string() => "0.0.4".to_string(),
                "mark2/dog2".to_string() => "0.0.13".to_string(),
            })
        );
    }
}

*/
