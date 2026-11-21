use std::collections::{BTreeMap, HashMap};

use crate::lexicon::Lexicon;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Relation {
    VerbObject,
    SubjectPredicate,
    AdjectiveNoun,
    AdverbPredicate,
    ClassifierNoun,
    Complement,
}

impl Relation {
    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "VO" | "V_O" | "V_HV_O" | "V_2HV_O" => Self::VerbObject,
            "SP" | "S_V" | "S_V_HV" | "S_V_2HV" => Self::SubjectPredicate,
            "AN" | "A_N" | "A_DE_N" => Self::AdjectiveNoun,
            "AP" | "D_V" | "D_A" => Self::AdverbPredicate,
            "CN" | "Q_N" => Self::ClassifierNoun,
            "PC" | "V_C" => Self::Complement,
            _ => return None,
        })
    }

    pub fn labels(self, reverse: bool) -> &'static str {
        match (self, reverse) {
            (Self::VerbObject, false) => "賓語",
            (Self::VerbObject, true) => "支配動詞",
            (Self::SubjectPredicate, false) => "謂語",
            (Self::SubjectPredicate, true) => "主語",
            (Self::AdjectiveNoun, false) => "修飾的名詞",
            (Self::AdjectiveNoun, true) => "定語",
            (Self::AdverbPredicate, false) => "修飾的謂語",
            (Self::AdverbPredicate, true) => "副詞修飾",
            (Self::ClassifierNoun, false) => "計量的名詞",
            (Self::ClassifierNoun, true) => "量詞",
            (Self::Complement, false) => "補語",
            (Self::Complement, true) => "搭配的動詞",
        }
    }
}

#[derive(Clone, Copy)]
pub enum Source {
    Education = 0,
    Wiki = 1,
    Taiwan = 2,
    Speech = 3,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Evidence {
    pub count: u32,
    pub association: f32,
}

#[derive(Default)]
struct Pair {
    evidence: [Evidence; 4],
    priority: u8,
    blocked: bool,
    reverse_sense: &'static str,
}

type Directions = BTreeMap<Group, Vec<Collocate>>;
pub type Entries = BTreeMap<String, Directions>;

#[derive(Default)]
pub struct Pairs {
    pairs: HashMap<(String, String), BTreeMap<Relation, Pair>>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Group {
    pub relation: Relation,
    pub reverse: bool,
    pub sense: &'static str,
}

#[derive(Debug)]
pub struct Collocate {
    pub partner: String,
    pub evidence: [Evidence; 4],
    priority: u8,
    generic: bool,
}

impl Pairs {
    pub fn len(&self) -> usize {
        self.pairs.values().map(BTreeMap::len).sum()
    }

    pub fn observe(
        &mut self,
        left: String,
        right: String,
        relation: Relation,
        source: Source,
        count: u32,
        association: f32,
    ) {
        let pair = self
            .pairs
            .entry((left, right))
            .or_default()
            .entry(relation)
            .or_default();
        let evidence = &mut pair.evidence[source as usize];
        // Script aliases and Hu parser variants can describe overlapping observations.
        if count > evidence.count || (count == evidence.count && association > evidence.association)
        {
            *evidence = Evidence { count, association };
        }
    }

    pub fn add_taiwan(&mut self, left: String, right: String, count: u32) -> bool {
        let Some(relations) = self.pairs.get_mut(&(left, right)) else {
            return false;
        };
        for pair in relations.values_mut() {
            let evidence = &mut pair.evidence[Source::Taiwan as usize];
            evidence.count = evidence.count.max(count);
        }
        true
    }

    pub fn review(
        &mut self,
        left: &str,
        right: &str,
        relation: Relation,
        action: &str,
        sense: &str,
    ) -> bool {
        let pair = self
            .pairs
            .entry((left.into(), right.into()))
            .or_default()
            .entry(relation)
            .or_default();
        match action {
            "block" => pair.blocked = true,
            "prefer" => pair.priority = 2,
            "keep" => pair.priority = 1,
            "group" => {}
            _ => return false,
        }
        match sense {
            "homework" => pair.reverse_sense = "課業",
            "operations" => pair.reverse_sense = "工作、操作",
            "" => {}
            _ => return false,
        }
        true
    }

    pub fn rank(self, lexicon: &Lexicon) -> Entries {
        let mut entries = Entries::new();
        for ((left, right), relations) in self.pairs {
            for (relation, pair) in relations {
                if !eligible(&left, &right, relation, &pair, lexicon) {
                    continue;
                }
                for (headword, partner, reverse, sense) in [
                    (&left, &right, false, ""),
                    (&right, &left, true, pair.reverse_sense),
                ] {
                    entries
                        .entry(headword.clone())
                        .or_default()
                        .entry(Group {
                            relation,
                            reverse,
                            sense,
                        })
                        .or_default()
                        .push(Collocate {
                            partner: partner.clone(),
                            evidence: pair.evidence,
                            priority: pair.priority,
                            generic: relation == Relation::AdverbPredicate
                                && matches!(
                                    left.as_str(),
                                    "也" | "都"
                                        | "就"
                                        | "又"
                                        | "还"
                                        | "便"
                                        | "在"
                                        | "将"
                                        | "已"
                                        | "曾"
                                        | "已经"
                                        | "还在"
                                ),
                        });
                }
            }
        }
        for groups in entries.values_mut() {
            for collocates in groups.values_mut() {
                rank_group(collocates);
            }
        }
        entries
    }
}

fn eligible(left: &str, right: &str, relation: Relation, pair: &Pair, lexicon: &Lexicon) -> bool {
    if pair.blocked {
        return false;
    }
    if pair.priority > 0 {
        return true;
    }
    if left == right || lexicon.is_name(left) || lexicon.is_name(right) {
        return false;
    }
    if relation == Relation::VerbObject
        && matches!(
            left,
            "是" | "有" | "没有" | "作为" | "成为" | "称为" | "叫做" | "用于" | "为"
        )
    {
        return false;
    }
    if matches!(
        right,
        "它" | "他" | "她" | "我们" | "你们" | "他们" | "什么" | "这个" | "那个" | "一些" | "一下"
    ) {
        return false;
    }
    let [ed, wiki, taiwan, speech] = pair.evidence;
    let known = lexicon.known(left) && lexicon.known(right);
    if !known && ed.count == 0 && taiwan.count == 0 && wiki.count < 12 && speech.count < 6 {
        return false;
    }

    match relation {
        Relation::VerbObject if matches!(left, "觉得" | "认为" | "以为") => return false,
        Relation::SubjectPredicate => {
            if matches!(
                right,
                "是" | "有"
                    | "没有"
                    | "在"
                    | "没"
                    | "就是"
                    | "使"
                    | "使得"
                    | "让"
                    | "成为"
                    | "还"
                    | "还有"
                    | "包括"
                    | "用"
                    | "为"
                    | "而"
                    | "和"
            ) || matches!(
                left,
                "间" | "中" | "上" | "下" | "方式" | "方面" | "之间" | "好像"
            ) {
                return false;
            }
            return (ed.count >= 3 && ed.association >= 0.8)
                || (wiki.count >= 6 && wiki.association >= 1.4)
                || (wiki.count >= 3 && wiki.association >= 0.8 && taiwan.count >= 40)
                || (speech.count >= 3 && known);
        }
        Relation::Complement if !is_complement(right) => return false,
        Relation::ClassifierNoun if !is_classifier(left) => return false,
        _ => {}
    }

    (ed.count >= 2 && ed.association >= -0.5)
        || (wiki.count >= 4 && wiki.association >= 0.3)
        || (wiki.count >= 2 && wiki.association >= 2.0 && (ed.count > 0 || taiwan.count > 0))
        || (ed.count > 0 && (wiki.count > 0 || taiwan.count > 0 || speech.count >= 2))
        || (speech.count >= 3 && known)
}

fn is_complement(word: &str) -> bool {
    matches!(
        word,
        "到" | "成"
            | "完"
            | "好"
            | "不好"
            | "掉"
            | "住"
            | "开"
            | "上"
            | "下"
            | "进"
            | "回"
            | "出"
            | "来"
            | "去"
            | "见"
            | "着"
            | "懂"
            | "错"
            | "对"
            | "清"
            | "清楚"
            | "明白"
            | "会"
            | "通"
            | "透"
            | "破"
            | "碎"
            | "坏"
            | "倒"
            | "给"
            | "在"
            | "至"
            | "为"
            | "于"
            | "向"
            | "走"
            | "动"
            | "起"
            | "稳"
            | "紧"
            | "干净"
            | "一致"
            | "得当"
            | "起来"
            | "过来"
            | "过去"
            | "回来"
            | "回去"
            | "出来"
            | "出去"
            | "进来"
            | "进去"
            | "下来"
            | "下去"
            | "上来"
            | "上去"
            | "回头"
            | "完毕"
            | "成功"
            | "满意"
            | "清洁"
            | "合格"
            | "干"
            | "熟"
            | "亮"
            | "醒"
            | "平"
            | "直"
            | "光"
            | "饱"
            | "累"
    )
}

fn is_classifier(word: &str) -> bool {
    matches!(
        word,
        "个" | "只"
            | "本"
            | "张"
            | "条"
            | "把"
            | "位"
            | "名"
            | "件"
            | "份"
            | "支"
            | "枝"
            | "根"
            | "颗"
            | "棵"
            | "朵"
            | "束"
            | "片"
            | "粒"
            | "块"
            | "辆"
            | "台"
            | "部"
            | "架"
            | "艘"
            | "匹"
            | "头"
            | "尾"
            | "峰"
            | "群"
            | "班"
            | "所"
            | "栋"
            | "幢"
            | "座"
            | "间"
            | "家"
            | "户"
            | "套"
            | "双"
            | "对"
            | "副"
            | "组"
            | "批"
            | "批次"
            | "卷"
            | "册"
            | "篇"
            | "封"
            | "则"
            | "期"
            | "集"
            | "首"
            | "曲"
            | "段"
            | "句"
            | "字"
            | "枚"
            | "面"
            | "幅"
            | "帧"
            | "声"
            | "次"
            | "回"
            | "趟"
            | "遍"
            | "场"
            | "顿"
            | "餐"
            | "餐次"
            | "轮"
            | "节"
            | "堂"
            | "门"
            | "科"
            | "节次"
            | "届"
            | "任"
            | "级"
            | "层"
            | "队"
            | "列"
            | "排"
            | "串"
            | "团"
            | "盒"
            | "包"
            | "袋"
            | "瓶"
            | "杯"
            | "碗"
            | "盘"
            | "锅"
            | "罐"
            | "桶"
            | "筐"
            | "箱"
            | "篮"
            | "勺"
            | "口"
            | "打"
            | "斤"
            | "公斤"
            | "克"
            | "升"
            | "笔"
            | "宗"
            | "项"
            | "款"
            | "桩"
            | "起"
            | "类"
            | "种"
            | "样"
            | "道"
            | "叠"
            | "滴"
            | "丝"
            | "缕"
            | "阵"
            | "股"
            | "抹"
            | "盏"
            | "顶"
            | "扇"
            | "池"
            | "堵"
            | "圈"
            | "轴"
            | "摞"
            | "沓"
            | "堆"
            | "摊"
            | "窝"
    )
}

fn rank_group(collocates: &mut Vec<Collocate>) {
    let weights = [1.2, 0.85, 0.95, 0.9];
    let mut ordered: Vec<usize> = (0..collocates.len()).collect();
    let mut score = vec![0.0_f32; collocates.len()];
    for (source, weight) in weights.into_iter().enumerate() {
        let strength = |index: usize| {
            let evidence = collocates[index].evidence[source];
            (evidence.count as f32).ln_1p() * (1.0 + evidence.association.clamp(0.0, 5.0) / 16.0)
        };
        ordered.sort_unstable_by(|&a, &b| {
            strength(b)
                .total_cmp(&strength(a))
                .then_with(|| collocates[a].partner.cmp(&collocates[b].partner))
        });
        let mut previous = f32::INFINITY;
        let mut rank = 0;
        for (position, &index) in ordered.iter().enumerate() {
            let value = strength(index);
            if value == 0.0 {
                break;
            }
            if value != previous {
                rank = position;
                previous = value;
            }
            // A lone observation is weaker evidence even when it ranks first in a small corpus.
            let confidence = (value / 2.0).min(1.0);
            score[index] += weight * confidence / (8 + rank) as f32;
        }
    }
    for (score, collocate) in score.iter_mut().zip(collocates.iter()) {
        if collocate.generic {
            *score -= 0.07;
        }
    }
    ordered.sort_unstable_by(|&a, &b| {
        collocates[b]
            .priority
            .cmp(&collocates[a].priority)
            .then_with(|| score[b].total_cmp(&score[a]))
            .then_with(|| collocates[a].partner.cmp(&collocates[b].partner))
    });
    let mut original: Vec<_> = std::mem::take(collocates).into_iter().map(Some).collect();
    collocates.extend(
        ordered
            .into_iter()
            .take(8)
            .map(|index| original[index].take().unwrap()),
    );
}
