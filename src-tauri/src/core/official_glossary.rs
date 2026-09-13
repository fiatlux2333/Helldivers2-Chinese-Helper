use std::sync::OnceLock;

const MAX_MATCHED_TERMS: usize = 16;
const OFFICIAL_GLOSSARY_TSV: &str = include_str!("official_glossary.tsv");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlossaryDirection {
    EnglishToChinese,
    ChineseToEnglish,
}

#[derive(Debug)]
struct OfficialTerm {
    category: &'static str,
    english: &'static str,
    chinese: &'static str,
    aliases: Vec<&'static str>,
}

pub fn prompt_with_official_terms(
    base_prompt: &str,
    text: &str,
    direction: GlossaryDirection,
) -> String {
    let terms = matched_terms(text, direction);
    if terms.is_empty() {
        return base_prompt.to_owned();
    }

    let heading = match direction {
        GlossaryDirection::EnglishToChinese => {
            "本条消息命中的官方术语候选（英文/别名 -> 官方简体中文）："
        }
        GlossaryDirection::ChineseToEnglish => {
            "本条消息命中的官方术语候选（官方简体中文 -> 英文原名）："
        }
    };
    let entries = terms
        .into_iter()
        .map(|term| match direction {
            GlossaryDirection::EnglishToChinese => {
                format!("[{}] {}={}", term.category, term.english, term.chinese)
            }
            GlossaryDirection::ChineseToEnglish => {
                format!("[{}] {}={}", term.category, term.chinese, term.english)
            }
        })
        .collect::<Vec<_>>()
        .join("；");

    let rule = match direction {
        GlossaryDirection::EnglishToChinese => {
            "使用规则：确认指向《绝地潜兵2》的敌人、战备、武器、分区、星球或其他专有名词时，必须使用官方简体中文译名；不是游戏术语时不要强行替换。"
        }
        GlossaryDirection::ChineseToEnglish => {
            "使用规则：确认指向《绝地潜兵2》的官方中文术语时，必须使用对应英文原名；不是游戏术语时不要强行替换。"
        }
    };

    format!("{base_prompt}\n\n{heading}\n{entries}\n{rule}")
}

fn official_terms() -> &'static [OfficialTerm] {
    static TERMS: OnceLock<Vec<OfficialTerm>> = OnceLock::new();
    TERMS.get_or_init(load_terms).as_slice()
}

fn load_terms() -> Vec<OfficialTerm> {
    OFFICIAL_GLOSSARY_TSV
        .lines()
        .skip(1)
        .filter_map(|line| {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                return None;
            }

            let mut fields = line.split('\t');
            let category = fields.next()?.trim();
            let english = fields.next()?.trim();
            let chinese = fields.next()?.trim();
            let aliases = fields
                .next()
                .unwrap_or_default()
                .split([';', '|', '、', '，'])
                .map(str::trim)
                .filter(|alias| !alias.is_empty())
                .collect::<Vec<_>>();

            if category.is_empty() || english.is_empty() || chinese.is_empty() {
                return None;
            }

            Some(OfficialTerm {
                category,
                english,
                chinese,
                aliases,
            })
        })
        .collect()
}

fn matched_terms(text: &str, direction: GlossaryDirection) -> Vec<&'static OfficialTerm> {
    let normalized_english = text.to_ascii_lowercase();
    let mut matches = official_terms()
        .iter()
        .filter_map(|term| {
            match_source(term, direction, text, &normalized_english).map(|source| (term, source))
        })
        .collect::<Vec<_>>();

    matches.sort_by_key(|(_, source)| std::cmp::Reverse(source.chars().count()));

    let mut selected: Vec<(&OfficialTerm, &'static str)> = Vec::new();
    for (term, source) in matches {
        if selected
            .iter()
            .any(|(_, selected_source)| source_is_covered(selected_source, source, direction))
        {
            continue;
        }
        selected.push((term, source));
        if selected.len() >= MAX_MATCHED_TERMS {
            break;
        }
    }

    selected.into_iter().map(|(term, _)| term).collect()
}

fn match_source(
    term: &'static OfficialTerm,
    direction: GlossaryDirection,
    original_text: &str,
    normalized_english: &str,
) -> Option<&'static str> {
    match direction {
        GlossaryDirection::EnglishToChinese => std::iter::once(term.english)
            .chain(term.aliases.iter().copied())
            .find(|source| {
                !source.is_empty() && normalized_english.contains(&source.to_ascii_lowercase())
            }),
        GlossaryDirection::ChineseToEnglish => {
            original_text.contains(term.chinese).then_some(term.chinese)
        }
    }
}

fn source_is_covered(
    selected_source: &'static str,
    source: &'static str,
    direction: GlossaryDirection,
) -> bool {
    match direction {
        GlossaryDirection::EnglishToChinese => selected_source
            .to_ascii_lowercase()
            .contains(&source.to_ascii_lowercase()),
        GlossaryDirection::ChineseToEnglish => selected_source.contains(source),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glossary_loads_the_exported_workbook_terms() {
        assert_eq!(official_terms().len(), 557);
    }

    #[test]
    fn spore_burst_strain_matches_the_strain_name_and_shorthand() {
        // The bare strain name and its common shorthands must map to the
        // official 孢裂变种, while full enemy names still win over the alias.
        let strain_prompt = prompt_with_official_terms(
            "base",
            "careful, spore burst incoming",
            GlossaryDirection::EnglishToChinese,
        );
        assert!(strain_prompt.contains("[敌人/目标] Spore Burst Strain=孢裂变种"));

        let shorthand_prompt = prompt_with_official_terms(
            "base",
            "spore strain bugs everywhere",
            GlossaryDirection::EnglishToChinese,
        );
        assert!(shorthand_prompt.contains("Spore Burst Strain=孢裂变种"));

        let enemy_prompt = prompt_with_official_terms(
            "base",
            "spore burst hunter on the left",
            GlossaryDirection::EnglishToChinese,
        );
        assert!(enemy_prompt.contains("[敌人/目标] Spore Burst Hunter=孢裂追猎虫"));
        assert!(!enemy_prompt.contains("Spore Burst Strain=孢裂变种"));

        let outgoing_prompt =
            prompt_with_official_terms("base", "孢裂变种来了", GlossaryDirection::ChineseToEnglish);
        assert!(outgoing_prompt.contains("[敌人/目标] 孢裂变种=Spore Burst Strain"));

        // Players colloquially call the Spore Burst strain "spore chargers"
        // (the bugs burst like a Charger group, and the strain has no Charger
        // of its own). The PLURAL form maps to the strain via the longest-
        // match dedup; the SINGULAR stays with the real Spore Charger enemy
        // (wiki: main horde), which must not be hijacked.
        let plural_prompt = prompt_with_official_terms(
            "base",
            "spore chargers incoming",
            GlossaryDirection::EnglishToChinese,
        );
        assert!(plural_prompt.contains("[敌人/目标] Spore Burst Strain=孢裂变种"));
        assert!(!plural_prompt.contains("Spore Charger=孢子强袭虫"));

        let singular_prompt = prompt_with_official_terms(
            "base",
            "a spore charger on the right",
            GlossaryDirection::EnglishToChinese,
        );
        assert!(singular_prompt.contains("Spore Charger=孢子强袭虫"));
        assert!(!singular_prompt.contains("Spore Burst Strain=孢裂变种"));
    }

    #[test]
    fn incoming_uses_the_longest_official_name() {
        let prompt = prompt_with_official_terms(
            "base",
            "charger behemoth on the right",
            GlossaryDirection::EnglishToChinese,
        );
        assert!(prompt.contains("[敌人/目标] Charger Behemoth=巨兽级强袭虫"));
        assert!(!prompt.contains("Charger=强袭虫"));
    }

    #[test]
    fn incoming_matches_stratagem_and_weapon_aliases() {
        let prompt = prompt_with_official_terms(
            "base",
            "meltagun and autocannon ready",
            GlossaryDirection::EnglishToChinese,
        );
        assert!(prompt.contains("[战备/武器] 40-K Meltagun=热熔枪"));
        assert!(prompt.contains("[战备/武器] AC-8 Autocannon=机炮"));
    }

    #[test]
    fn outgoing_adds_only_terms_present_in_the_message() {
        let prompt = prompt_with_official_terms(
            "base",
            "右边有追猎虫，带热熔枪",
            GlossaryDirection::ChineseToEnglish,
        );
        assert!(prompt.contains("[敌人/目标] 追猎虫=Hunter"));
        assert!(prompt.contains("[战备/武器] 热熔枪=40-K Meltagun"));
        assert!(!prompt.contains("吐酸泰坦=Bile Titan"));
    }

    #[test]
    fn unrelated_chat_does_not_expand_the_prompt() {
        assert_eq!(
            prompt_with_official_terms(
                "base",
                "go left and wait",
                GlossaryDirection::EnglishToChinese
            ),
            "base"
        );
    }
}
