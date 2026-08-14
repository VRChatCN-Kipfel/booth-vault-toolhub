//! BOOTH 类目 → 中文分类。
//!
//! 映射表来自两个历史分支的合并结果：
//! `ヘアー/髪/ヘア → 3D发型`、`アバター → 头像`（去重）、
//! `3Dシェーダー・マテリアル → 着色器`、`3Dモーション → 3D动作`、
//! `ソフトウェア → 软件`（原统一版独有，保留）。

use phf::phf_map;

/// BOOTH 类目 → 中文（合并去重固化，53 键）。任何修改必须同步 GUI/CLI/MCP 三端。
pub static CATEGORY_MAP: phf::Map<&'static str, &'static str> = phf_map! {
    "3Dアバター" => "3D头像",
    "3D衣装・アクセサリー" => "3D服饰",
    "3Dモデル" => "3D模型",
    "3Dモデル（その他）" => "3D模型（其他）",
    "3D装飾品" => "3D饰品",
    "3D環境・ワールド" => "3D环境",
    "3Dキャラクター" => "3D角色",
    "3D小道具" => "3D道具",
    "アバター" => "头像",
    "アバターアイテム" => "头像物品",
    "アバターギミック" => "头像机关",
    "アクセサリ" => "饰品",
    "アクセサリー" => "配饰",
    "衣装・アクセサリー" => "服饰饰品",
    "衣装" => "服饰",
    "髪" => "3D发型",
    "ヘアー" => "3D发型",
    "ヘア" => "3D发型",
    "バッジ" => "徽章",
    "モーション" => "动作",
    "ギミック" => "机关",
    "リギング" => "绑定",
    "テクスチャ" => "贴图",
    "テクスチャ素材" => "贴图素材",
    "シェーダー" => "着色器",
    "エフェクト" => "特效",
    "ツール" => "工具",
    "ツール・プラグイン" => "工具插件",
    "物理" => "物理",
    "VR" => "VR",
    "3Dモーション・アニメーション" => "3D动作",
    "3Dツール・システム" => "3D工具",
    "3D衣装" => "3D服饰",
    "3Dモーション" => "3D动作",
    "3Dシェーダー・マテリアル" => "着色器",
    "3Dテクスチャ" => "3D贴图",
    "テクスチャ・素材" => "贴图素材",
    "ﾓｼﾞｬｰﾙｱｲﾃﾑ" => "AR物品",
    "音声" => "语音",
    "効果音・SE" => "音效",
    "BGM" => "BGM",
    "素材" => "素材",
    "イラスト" => "插画",
    "漫画" => "漫画",
    "小説" => "小说",
    "ポスター" => "海报",
    "その他" => "其他",
    "ゲーム" => "游戏",
    "ゲーム関連商品" => "游戏相关",
    "フリーゲーム" => "免费游戏",
    "素材データ" => "素材数据",
    "音楽" => "音乐",
    "ソフトウェア" => "软件",
};

/// 父类精确映射（key 取自 JSON category.parent）。
pub static CATEGORY_PARENT_MAP: phf::Map<&'static str, &'static str> = phf_map! {
    "3Dモデル" => "3D模型",
    "ゲーム" => "游戏",
    "アバター" => "头像",
};

/// 父类属于以下时，子分类强制加 3D 前缀（与归档目录结构对齐）。
const THREED_PARENTS: &[&str] = &[
    "3Dモデル",
    "3Dモデル（その他）",
    "3D衣装・アクセサリー",
    "アバター",
];

/// BOOTH 类目 → 中文。优先精确；退回父级；再退回保留日文原名（绝不臆造）。
///
/// 父级 3D 前缀规则：当父类是 3D 系（3Dモデル / 3Dモデル（その他）/ アバター），
/// 但 cat_name 映射出的中文不含「3D」前缀时，自动补「3D」前缀。
pub fn classify(cat_name: &str, cat_parent: &str) -> String {
    if cat_name.is_empty() {
        return "未分类".to_string();
    }
    let result = if let Some(v) = CATEGORY_MAP.get(cat_name) {
        v
    } else if !cat_parent.is_empty() {
        if let Some(v) = CATEGORY_MAP.get(cat_parent) {
            v
        } else if let Some(v) = CATEGORY_PARENT_MAP.get(cat_parent) {
            v
        } else {
            cat_name
        }
    } else {
        cat_name
    };
    if THREED_PARENTS.contains(&cat_parent) && !result.starts_with("3D") {
        return format!("3D{result}");
    }
    result.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_exact() {
        assert_eq!(classify("髪", ""), "3D发型");
        assert_eq!(classify("ヘアー", ""), "3D发型");
        assert_eq!(classify("アバター", ""), "头像");
        assert_eq!(classify("ソフトウェア", ""), "软件");
        assert_eq!(classify("ゲーム", ""), "游戏");
    }

    #[test]
    fn classify_empty() {
        assert_eq!(classify("", ""), "未分类");
    }

    #[test]
    fn classify_fallback_parent() {
        assert_eq!(classify("衣装", "3Dモデル"), "3D服饰");
        assert_eq!(classify("髪", "3Dモデル"), "3D发型");
        assert_eq!(classify("アクセサリー", "3D衣装・アクセサリー"), "3D配饰");
    }

    #[test]
    fn classify_keep_original() {
        assert_eq!(classify("未知类目", ""), "未知类目");
        assert_eq!(classify("未知类目", "不存在父类"), "未知类目");
    }

    #[test]
    fn classify_no_dup_3d_prefix() {
        // 已是 3D 开头不重复加
        assert_eq!(classify("3Dモデル", "3Dモデル"), "3D模型");
        assert_eq!(classify("髪", "3Dモデル"), "3D发型");
        // 映射值非 3D 开头 + 父类在 3D 白名单 → 自动补前缀
        assert_eq!(classify("3Dシェーダー・マテリアル", "3Dモデル"), "3D着色器");
    }
}
