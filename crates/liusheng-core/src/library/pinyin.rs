use pinyin::ToPinyin;

/// 归一化：只留字母数字与汉字，统一小写。查询词和索引两侧都过这一步。
pub fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// 生成检索串：归一化原文、全拼、首字母三段以换行相连。
/// 例："林俊杰" -> "林俊杰\nlinjunjie\nljj"，查询任一形式都能以子串命中。
pub fn search_blob(text: &str) -> String {
    let plain = normalize(text);
    let mut full = String::new();
    let mut initials = String::new();
    for c in text.chars() {
        match c.to_pinyin() {
            Some(py) => {
                full.push_str(py.plain());
                if let Some(f) = py.first_letter().chars().next() {
                    initials.push(f);
                }
            }
            None => {
                if c.is_alphanumeric() {
                    for lc in c.to_lowercase() {
                        full.push(lc);
                        initials.push(lc);
                    }
                }
            }
        }
    }
    format!("{plain}\n{full}\n{initials}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinyin_full_and_initials() {
        let blob = search_blob("林俊杰");
        assert!(blob.contains("linjunjie"));
        assert!(blob.contains("ljj"));
        assert!(blob.contains("林俊杰"));
    }

    #[test]
    fn mixed_han_and_latin() {
        let blob = search_blob("Jay 周杰伦");
        assert!(blob.contains("zhoujielun"));
        assert!(blob.contains("jay"));
        // 首字母段：jay 原样 + 周杰伦 zjl
        assert!(blob.contains("jayzjl"));
    }

    #[test]
    fn normalize_strips_punct() {
        assert_eq!(normalize("起风了（Live）"), "起风了live");
    }
}
