//! 与 Python 旧实现输出 diff 对拍（golden tests）。
//!
//! 黄金数据由 `booth_common.py` 实机运行生成（2026-08-15），
//! 覆盖 sanitize_filename / sanitize_query / extract_version_tag。
//! 若行为需调整，先更新 Python 基线再重新生成本文件。

use engine::clean::{extract_version_tag, sanitize_filename, sanitize_query};

#[test]
fn golden_sanitize_filename() {
    let cases: &[(&str, &str)] = &[
        (
            "メカ弾エフェクトVer_2.00.unitypackage",
            "メカ弾エフェクトVer_2.00.unitypackage",
        ),
        ("雪女✨エフェクト.rar", "雪女エフェクト.rar"),
        ("Star-Tiara_v1.0.zip", "Star-Tiara_v1.0.zip"),
        ("LunariaPaperFan (1).zip", "LunariaPaperFan (1).zip"),
        ("🎉Party_2024🎉.zip", "Party_2024.zip"),
        (
            "SimpleJoinAlert_v100.unitypackage",
            "SimpleJoinAlert_v100.unitypackage",
        ),
        (
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.zip",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ),
        ("café_theme.zip", "café_theme.zip"),
        ("ライン素材【無料】.zip", "ライン素材【無料】.zip"),
        (
            "MyWorld_vrchat_1.2.unitypackage",
            "MyWorld_vrchat_1.2.unitypackage",
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(sanitize_filename(input), *expected, "input: {input}");
    }
}

#[test]
fn golden_sanitize_query() {
    let cases: &[(&str, &[&str])] = &[
        (
            "メカ弾エフェクトVer_2.00.unitypackage",
            &[
                "メカ弾エフェクトVer 2.00",
                "メカ弾エフェクト",
                "メカ弾エフェクトVer",
            ],
        ),
        (
            "雪女✨エフェクト.rar",
            &["雪女✨エフェクト", "雪女エフェクト"],
        ),
        (
            "Star-Tiara_v1.0.zip",
            &["Star-Tiara v1.0", "Star-Tiara", "Tiara", "Star"],
        ),
        (
            "LunariaPaperFan (1).zip",
            &["LunariaPaperFan", "Lunaria Paper Fan"],
        ),
        ("🎉Party_2024🎉.zip", &["🎉Party 2024🎉", "Party"]),
        (
            "SimpleJoinAlert_v100.unitypackage",
            &[
                "SimpleJoinAlert v100",
                "Simple Join Alert v100",
                "SimpleJoinAlert",
                "v100",
            ],
        ),
        (
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.zip",
            &[
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ],
        ),
        ("café_theme.zip", &["café theme", "theme"]),
        ("ライン素材【無料】.zip", &["ライン素材"]),
        (
            "MyWorld_vrchat_1.2.unitypackage",
            &[
                "MyWorld vrchat 1.2",
                "My World vrchat 1.2",
                "MyWorld vrchat",
                "MyWorld",
                "vrchat",
            ],
        ),
    ];
    for (input, expected) in cases {
        let got = sanitize_query(input);
        assert_eq!(got, *expected, "input: {input}");
    }
}

#[test]
fn golden_extract_version_tag() {
    let cases: &[(&str, &str)] = &[
        ("メカ弾エフェクトVer_2.00.unitypackage", "Ver_2.00"),
        ("雪女✨エフェクト.rar", ""),
        ("Star-Tiara_v1.0.zip", "Ver_1.0"),
        ("LunariaPaperFan (1).zip", ""),
        ("🎉Party_2024🎉.zip", ""),
        ("SimpleJoinAlert_v100.unitypackage", "Ver_100"),
        (
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.zip",
            "",
        ),
        ("café_theme.zip", ""),
        ("ライン素材【無料】.zip", ""),
        ("MyWorld_vrchat_1.2.unitypackage", "Ver_1.2"),
    ];
    for (input, expected) in cases {
        assert_eq!(extract_version_tag(input), *expected, "input: {input}");
    }
}
