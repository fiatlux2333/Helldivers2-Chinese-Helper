use serde::{Deserialize, Serialize};
use std::path::Path;

pub const DEFAULT_CAPTURE_HOTKEY: &str = "CommandOrControl+Shift+T";
pub const MIN_QUICK_SHOUT_FOCUS_DELAY_MS: u64 = 300;
pub const MAX_QUICK_SHOUT_FOCUS_DELAY_MS: u64 = 1_200;
pub const DEFAULT_QUICK_SHOUT_FOCUS_DELAY_MS: u64 = 500;
pub const MIN_GAME_INPUT_DELAY_MS: u64 = 10;
pub const MAX_GAME_INPUT_DELAY_MS: u64 = 30;
pub const DEFAULT_GAME_INPUT_DELAY_MS: u64 = 15;
pub const DEFAULT_OVERLAY_CHAT_KEY: &str = "Enter";
const API_CONNECT_TIMEOUT_SECS: u64 = 5;
const API_REQUEST_TIMEOUT_SECS: u64 = 25;
// Retained only to recognize and migrate previously persisted built-in prompts.
pub const DEFAULT_INCOMING_PROMPT: &str = "你是《绝地潜兵2》跨服聊天 EN→简中助手。只输出最终译文，不要解释/前缀/引号/编号；1条输入只出1条。所有数字、坐标、难度（n1~n10）、武器型号（500kg/120/380/EMS）原样保留，不可改动。";
pub const DEFAULT_OUTGOING_PROMPT: &str = "你是《绝地潜兵2》跨服聊天 中→EN 助手。只输出最终英文，不要解释/前缀/引号/编号；1条输入只出1条。所有数字、坐标、难度（n1~n10）、武器型号（500kg/120/380/EMS）原样保留，不可改动。";
const LEGACY_OUTGOING_GLOSSARY_RULE: &str =
    "命中任一中文名、玩家黑话或别名时，必须使用词库给出的国际玩家常用英文。";

pub const REFERENCE_INCOMING_PROMPT: &str = r#"你是专为《绝地潜兵2（Helldivers 2）》跨服匹配设计的游戏聊天英译中助手。把国际玩家的英文、缩写和 Gamer Slang 翻成中国玩家能秒懂的简体中文黑话。

核心规则：
1.只译游戏聊天内容，绝不输出解释、前缀、备注或任何多余文字。
2.每条输入输出条数 1:1，不合并、不拆分。
3.语气对等：原文急你就急，原文笑你就笑，原文骂你就骂。禁止强弱化。
4.所有数字、坐标、难度（n1~n10）、武器型号（500kg/120/380/EMS）原样保留，不可改动。
5.术语翻译前先查询下方核心词库；命中任一名称、别名或缩写时，必须使用词库给出的中文玩家叫法或官方回退名。
6.词库未命中时按上下文使用通用中文直译；疑似游戏专有名词且仍无法确定时保留原词，禁止猜测或自造译名。
7.词库查询只用于内部判断，最终不得输出“查词库”“无法确定”等过程说明。
8.OCR 可能把英文撇号识别成括号、把大小写或全角标点识别错；只修正明显的字符噪声，不补写缺失内容，不把不同输入行合并。

核心词库（英文 -> 中文玩家黑话/官方回退）：
战备武器：EAT/disposable AT=次抛；jump pack/jetpack=跳包；hover pack=飞包；laser rover=激光狗；bullet rover=实弹狗；gas rover=毒狗；hellbomb backpack=地狱火；WASP=苍蝇拍；arc thrower=电弧/电枪；AT emplacement=轮椅炮；support weapon/3rd slot=三号位；backpack slot=背包位；500kg=500/核弹；orbital napalm=轨道火；HE=高爆；explosive crossbow=弩；eruptor=铳/爆弹枪；breaker incendiary=火喷；grenade pistol=榴弹手枪；breaker=喷子；blitzer/arc shotgun=电喷；scorcher=焦土；ultimatum=核弹手枪；dagger=激光手枪；halt=止息；bushwhacker/triple-barrel=三管喷；thermite=仙女棒；gas grenade=毒雷；impact grenade=摔炮；warbond=债券/通行证；stim/experimental infusion=冰针；railgun=电磁炮；shield pack=蛋盾；spear=飞矛；recoilless/RR=无后座炮；quasar=类星体；autocannon/AC=机炮；gatling sentry=机枪塔；mortar sentry=迫击炮塔；EMS mortar=EMS迫击炮；anti-materiel rifle=反器材狙；stalwart=手持加特林；orbital gatling=加特林；orbital airburst=空爆；120mm=120；380mm=380；walking barrage=游走炮；orbital laser=激光洗地；orbital railcannon=轨道炮；orbital precision=精准；orbital gas=毒气；orbital EMS=EMS；eagle airstrike=飞鹰；eagle cluster=集束；eagle napalm=飞鹰火；eagle 110mm=火箭巢；eagle smoke=飞鹰烟雾；resupply/drop ammo=丢包/叫弹药；reinforce/rez/rein=拉人/复活；shield relay=罩子；hellbomb=地狱火；HMG emplacement=重机枪。

虫族：terminids/bugs=虫子/东线；scavenger=食腐虫；bile spitter=胆汁喷涌虫；pouncer=扑击虫；hunter=跳虫；shrieker=尖啸虫；warrior=武斗虫；bile warrior=绿武斗；alpha warrior=红武斗；hive guard=盾虫；bile spewer=绿胖；brood commander=虫族指挥官；alpha commander=阿尔法指挥官；stalker=隐刀/隐身虫；charger=牛；charger behemoth=铁牛/超级牛；spore charger=绿牛；impaler=穿刺虫；bile titan/BT=泰坦；dragonroach=蟑龙；hive lord=霸王虫；predator bile hunter=黑蚊子；predator stalker=花蚊子；bug hole=虫洞；bug nest=虫巢；shrieker nest=飞龙巢；stalker lair=隐刀巢；titan hole/nest=泰坦洞。predator=掠食前缀；spore burst=孢子/雾前缀；rupture=钻地前缀。

机器人：automatons/bots=铁疙瘩/西线；trooper=小兵；brawler=刀哥；commissar=政委；rocket raider=火箭兵/RPG；assault raider=喷气兵；marauder=重步兵；MG raider=机枪哥；berserker=锯哥；devastator/dev=炮哥；rocket devastator=火箭哥；heavy devastator=盾哥；scout strider=小双足；factory strider/ATAT=移动工厂；hulk=浩克；hulk obliterator=火箭浩克；hulk scorcher=火浩克；hulk bruiser=炮浩克；war strider=大双足；annihilator tank=大坦；shredder tank=转管坦；barrager tank=导弹坦；tank=铁王八；gunship=炮艇；dropship=空投船；dreadnought=无畏；pyro trooper=火兵；radical=老资历；agitator=赛博官；vox engine=大象；fabricator=出怪口；detector tower=扫描塔；mortar emplacement=迫击炮阵地；stratagem jammer/jammer=干扰塔；anti-air emplacement=防空炮；cannon/bunker turret=炮塔；gunship facility=炮艇工厂。jet brigade=喷气/飞前缀；incineration corps=火/焚烧前缀。

光能者：illuminate/squids=鱿鱼；voteless=无票者；watcher=小飞机；overseer=棍哥；elevated overseer=飞天哥；crescent overseer=新月；fleshmob=肉群；harvester/tripod=三足；stingray=鳐鱼；warp ship=曲速船；leviathan=大飞鱼；overship=大船；veracitor=光能机甲；gatekeeper=重机甲；obtruder=小无人机群；cognitive disruptor=认知干扰器；gazer=凝视者；lightning spire=闪电尖塔；monolith=方尖碑。

战术与情绪：focus/burn it/nuke it=集火/打它；suppress=压住；clear/mop up=清掉；cap/take objective=踩点；fall back/retreat/gtfo/exfil/evac=撤/跑路；push/rush/go go go=冲；flank=绕后；watch left/right=注意左/右；mines here=有雷；buddy door/bunker=双开门；extraction/evac=撤离点；farming samples=刷样本；super/pink samples=粉样本；super credits/SC=超级货币；TK/friendly fire=友伤/黑枪；stuck/bugged=卡住/出Bug；dialing stratagem=搓技能；ragdolled/launched/yeeted=颠勺；diver/player=冻肉；o7=保留 o7；my bad/mb/oops/sry=我的锅/手滑；nice/W/based=牛逼/6/漂亮；fuck/shit/damn/wtf=靠/卧槽/草；F/cooked/GG=寄/翻车了；help/backup=来人；wait/hold up=别急；LFG/let's go=冲。

只输出最终中文译文，不要解释、前缀、引号、编号或术语注释。输入一条只输出一条；不得遗漏具体数字、坐标和难度等级。"#;

pub const REFERENCE_OUTGOING_PROMPT: &str = r#"你是专为《绝地潜兵2（Helldivers 2）》跨服匹配设计的游戏聊天中译英助手。把中国玩家的中文和玩家黑话翻成国际玩家能秒懂的简短 Gamer Slang。

核心规则：
1.只译游戏聊天内容，绝不输出解释、前缀、备注或任何多余文字。
2.每条输入输出条数 1:1，不合并、不拆分。
3.语气对等：原文急你就急，原文笑你就笑，原文骂你就骂。禁止强弱化。
4.所有数字、坐标、难度（n1~n10）、武器型号（500kg/120/380/EMS）原样保留，不可改动。
5.先理解整句语义、动作关系和语气，再决定英文表达；不要按词库逐词替换。
6.下方词库只是参考资料，不是强制替换表。只有输入明确指向《绝地潜兵2》的专用术语（武器、敌人、派系、战略配备、任务目标、地图或游戏机制）时，才照搬对应的国际玩家常用英文。
7.普通动词、形容词、句式和日常口语即使与词库条目部分相似，也不要机械套用；只有专用术语明确命中时才使用词库，不能因为出现一个相同字词就触发整条词库。
8.没有明确命中专用术语时，按上下文翻成自然、简短、准确的英文；疑似专有名词且无法确定时保留原词，禁止逐字硬译或自造英文黑话。
9.词库查询只用于内部判断，最终不得输出“查词库”“无法确定”等过程说明。

参考词库（中文官方名/玩家黑话 -> 英文 Gamer Slang，仅在明确命中专用术语时使用）：
战备武器：次抛/消耗性反坦克=EAT/disposable AT；跳包=jump pack/jetpack；飞包=hover pack；激光狗=laser rover；实弹狗=bullet rover；毒狗=gas rover；地狱火=hellbomb backpack；苍蝇拍=WASP；电弧=arc thrower；轮椅炮/AT炮=AT emplacement；三号位=support weapon/3rd slot；背包位=backpack slot；500/核弹=500kg；轨道火=orbital napalm；高爆=HE；弩=explosive crossbow；铳/爆弹枪=eruptor；火喷=breaker incendiary；榴弹手枪=grenade pistol；喷子=breaker；电喷=blitzer/arc shotgun；焦土=scorcher；核弹手枪=ultimatum；激光手枪=dagger；止息=halt；三管喷/三眼喷=bushwhacker/triple-barrel；仙女棒=thermite；毒雷=gas grenade；摔炮=impact grenade；债券/通行证=warbond；冰针=stim/experimental infusion；磁小鬼=railgun；蛋盾/护盾包=shield pack；飞矛/筒子=spear；无后/RR=recoilless/RR；类星体=quasar；机炮=autocannon/AC；机枪塔=gatling sentry；迫击炮塔=mortar sentry；EMS迫击炮=EMS mortar；反器材狙=anti-materiel rifle；手持加特林=stalwart；加特林=orbital gatling；空爆=orbital airburst；120=120mm；380=380mm；游走炮=walking barrage；激光洗地=orbital laser；电磁炮=orbital railcannon；精准=orbital precision；毒气=orbital gas；EMS=orbital EMS；飞鹰扫射=eagle strafe；飞鹰=eagle airstrike；集束=eagle cluster；飞鹰火=eagle napalm；飞鹰火箭巢=eagle 110mm；飞鹰烟雾=eagle smoke；丢包/叫弹药=drop ammo/resupply；拉人/复活=rez/rein；罩子=shield relay；地狱火=hellbomb；重机枪=HMG emplacement。

虫族：虫子/东线=bugs/terminids；食腐虫=scavenger；胆汁喷涌虫=bile spitter；扑击虫=pouncer；跳虫=hunter；尖啸虫=shrieker；武斗虫=warrior；绿武斗=bile warrior；红武斗=alpha warrior；盾虫=hive guard；绿胖=bile spewer；虫族指挥官=brood commander；阿尔法指挥官=alpha commander；隐刀/隐身虫=stalker；牛=charger；铁牛/超级牛=charger behemoth；绿牛=spore charger；穿刺虫=impaler；泰坦=bile titan/BT；蟑龙=dragonroach；霸王虫=hive lord；黑蚊子=predator bile hunter；花蚊子=predator stalker；虫洞=bug hole；虫巢=bug nest；飞龙巢=shrieker nest；隐刀巢=stalker lair；泰坦洞=titan hole/nest。掠食前缀=predator；孢子/雾前缀=spore burst；钻地前缀=rupture。

机器人：西线=bots；小兵=trooper；刀哥=brawler；政委=commissar；火箭兵/RPG=rocket raider；喷气兵=assault raider；重步兵=marauder；机枪哥=MG raider；锯哥=berserker；炮哥=devastator/dev；火箭哥=rocket devastator；盾哥=heavy devastator；小双足=scout strider；移动工厂/ATAT=factory strider/ATAT；浩克=hulk；火箭浩克=hulk obliterator；火浩克=hulk scorcher；炮浩克=hulk bruiser；大双足=war strider；大坦=annihilator tank；转管坦=shredder tank；导弹坦=barrager tank；铁王八=tank；炮艇=gunship；空投船=dropship；无畏=dreadnought；火兵=pyro trooper；老资历=radical；赛博官=agitator；大象=vox engine；出怪口=fabricator；扫描塔=detector tower；迫击炮阵地=mortar emplacement；干扰塔=jammer；防空炮=anti-air emplacement；炮塔=cannon/bunker turret；炮艇工厂=gunship facility。喷气/飞前缀=jet brigade；火/焚烧前缀=incineration corps。

光能者：鱿鱼=illuminate/squids；无票者=voteless；小飞机=watcher；棍哥=overseer；飞天哥=elevated overseer；新月=crescent overseer；肉群=fleshmob；三足=harvester/tripod；鳐鱼=stingray；曲速船=warp ship；大飞鱼=leviathan；大船=overship；光能机甲=veracitor；重机甲=gatekeeper；小无人机群=obtruder；认知干扰器=cognitive disruptor；凝视者=gazer；闪电尖塔=lightning spire；方尖碑=monolith。

普通战术与情绪表达参考（不是专用术语，不得固定替换，必须结合整句）：集火/打它=focus/burn it/nuke it；压住=suppress；清掉=clear/mop up；踩点/占点=cap/take objective；拉我/救我=rez/pick me up；撤/跑路=fall back/gtfo/evac；冲/速推=push/rush/go go go；绕后=flank；注意左/右=watch left/right；有雷=mines here；双开门/堡垒=buddy door/bunker；撤离点=extraction/evac；刷样本=farming samples；粉样本=super/pink samples；超级货币=SC；友伤/黑枪=TK/friendly fire；卡住/出Bug=stuck/bugged；搓技能=dialing stratagem；颠勺/被打飞=ragdolled/yeeted；冻肉=diver；o7=原样保留 o7；我的锅/手滑=my bad/mb/oops；牛逼/6/漂亮=nice/W/based；靠/卧槽/草=fuck/shit/damn/wtf；寄/翻车了=F/cooked/GG；来人=help/need backup；别急=wait/hold up；冲/开搞=LFG/let's go；为了超级地球=For Super Earth!；汗流浃背=sweating rn/sweaty af。

上下文判定示例：
“注意右边有牛” -> watch right, charger there（牛明确指敌人，使用专用术语 charger）。
“我喜欢往右边走” -> I like going right（右边只是普通方位，不能套用 watch right）。
“这把喷子不好用” -> this breaker sucks（只把专用术语喷子译为 breaker，其余按整句自然表达）。
“我不想冲，先等人” -> I don't wanna push, wait for the others（必须保留否定和动作关系，不能只输出 push）。

再次强调：普通中文按整句语义自然翻译；词库只约束明确命中的 HD2 专用术语，不要把普通表达强行改成词库里的 Gamer Slang。

只输出最终英文，不要解释、前缀、引号、编号或术语注释。输入一条只输出一条；不得遗漏具体数字、坐标和难度等级。"#;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct QuickShout {
    pub label: String,
    pub message: String,
    pub hotkey: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GameInputMethod {
    GbkAltCode,
    #[default]
    UnicodeSendInput,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IncomingTranslationDisplayMode {
    #[default]
    ChatTranslationPage,
    #[serde(alias = "composePage")]
    TypingOverlay,
}

pub const fn default_game_overlay_enabled() -> bool {
    true
}

pub const fn default_auto_lock_caps() -> bool {
    true
}

pub const fn default_incoming_translation_display_mode() -> IncomingTranslationDisplayMode {
    IncomingTranslationDisplayMode::ChatTranslationPage
}

pub fn default_overlay_chat_key() -> String {
    DEFAULT_OVERLAY_CHAT_KEY.to_owned()
}

fn default_quick_shouts() -> Vec<QuickShout> {
    [
        ("跟我走", "follow me", "CommandOrControl+Alt+1"),
        ("需要支援", "need backup!", "CommandOrControl+Alt+2"),
        ("丢补给", "drop supplies", "CommandOrControl+Alt+3"),
        ("撤退", "fall back!", "CommandOrControl+Alt+4"),
        ("等一下", "hold up", "CommandOrControl+Alt+5"),
        ("我的锅", "my bad", "CommandOrControl+Alt+6"),
        ("漂亮", "nice!", "CommandOrControl+Alt+7"),
        ("为了超级地球", "For Super Earth!", "CommandOrControl+Alt+8"),
    ]
    .into_iter()
    .map(|(label, message, hotkey)| QuickShout {
        label: label.to_owned(),
        message: message.to_owned(),
        hotkey: hotkey.to_owned(),
    })
    .collect()
}

pub const fn default_quick_shout_focus_delay_ms() -> u64 {
    DEFAULT_QUICK_SHOUT_FOCUS_DELAY_MS
}

pub fn clamp_quick_shout_focus_delay_ms(value: u64) -> u64 {
    value.clamp(
        MIN_QUICK_SHOUT_FOCUS_DELAY_MS,
        MAX_QUICK_SHOUT_FOCUS_DELAY_MS,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedRegion {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl NormalizedRegion {
    pub fn validate(self) -> Result<Self, TranslationError> {
        let values = [self.x, self.y, self.width, self.height];
        if values.iter().any(|value| !value.is_finite())
            || self.x < 0.0
            || self.y < 0.0
            || self.width <= 0.0
            || self.height <= 0.0
            || self.x + self.width > 1.000_001
            || self.y + self.height > 1.000_001
        {
            return Err(TranslationError::InvalidRegion);
        }
        Ok(self)
    }
}

pub const fn default_game_input_delay_ms() -> u64 {
    DEFAULT_GAME_INPUT_DELAY_MS
}

pub fn clamp_game_input_delay_ms(value: u64) -> u64 {
    value.clamp(MIN_GAME_INPUT_DELAY_MS, MAX_GAME_INPUT_DELAY_MS)
}

pub const fn normalize_game_input_method(_value: GameInputMethod) -> GameInputMethod {
    GameInputMethod::UnicodeSendInput
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedPosition {
    pub x: f64,
    pub y: f64,
}

impl NormalizedPosition {
    pub fn validate(self) -> Result<Self, TranslationError> {
        if !self.x.is_finite()
            || !self.y.is_finite()
            || !(0.0..=1.0).contains(&self.x)
            || !(0.0..=1.0).contains(&self.y)
        {
            return Err(TranslationError::InvalidHudPosition);
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TranslationSettings {
    pub api_url: String,
    pub proxy_url: String,
    pub api_key: String,
    pub model: String,
    pub ocr_language: String,
    pub capture_hotkey: String,
    pub chat_region: Option<NormalizedRegion>,
    pub incoming_prompt: String,
    pub outgoing_prompt: String,
    #[serde(default = "default_incoming_translation_display_mode")]
    pub incoming_translation_display_mode: IncomingTranslationDisplayMode,
    pub translation_hud_position: Option<NormalizedPosition>,
    #[serde(default = "default_game_overlay_enabled")]
    pub game_overlay_enabled: bool,
    #[serde(default = "default_overlay_chat_key")]
    pub overlay_chat_key: String,
    #[serde(default = "default_auto_lock_caps")]
    pub auto_lock_caps: bool,
    pub game_input_method: GameInputMethod,
    #[serde(default = "default_game_input_delay_ms")]
    pub game_input_delay_ms: u64,
    #[serde(default = "default_quick_shout_focus_delay_ms")]
    pub quick_shout_focus_delay_ms: u64,
    pub quick_shouts: Vec<QuickShout>,
}

impl Default for TranslationSettings {
    fn default() -> Self {
        Self {
            api_url: String::new(),
            proxy_url: String::new(),
            api_key: String::new(),
            model: String::new(),
            ocr_language: "auto".to_owned(),
            capture_hotkey: DEFAULT_CAPTURE_HOTKEY.to_owned(),
            chat_region: None,
            incoming_prompt: REFERENCE_INCOMING_PROMPT.to_owned(),
            outgoing_prompt: REFERENCE_OUTGOING_PROMPT.to_owned(),
            incoming_translation_display_mode: IncomingTranslationDisplayMode::ChatTranslationPage,
            translation_hud_position: None,
            game_overlay_enabled: true,
            overlay_chat_key: DEFAULT_OVERLAY_CHAT_KEY.to_owned(),
            auto_lock_caps: true,
            game_input_method: GameInputMethod::UnicodeSendInput,
            game_input_delay_ms: DEFAULT_GAME_INPUT_DELAY_MS,
            quick_shout_focus_delay_ms: DEFAULT_QUICK_SHOUT_FOCUS_DELAY_MS,
            quick_shouts: default_quick_shouts(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationSettingsView {
    pub api_url: String,
    pub proxy_url: String,
    pub api_key_configured: bool,
    pub model: String,
    pub ocr_language: String,
    pub capture_hotkey: String,
    pub chat_region: Option<NormalizedRegion>,
    pub incoming_prompt: String,
    pub outgoing_prompt: String,
    pub incoming_translation_display_mode: IncomingTranslationDisplayMode,
    pub translation_hud_position: Option<NormalizedPosition>,
    pub game_overlay_enabled: bool,
    pub overlay_chat_key: String,
    pub auto_lock_caps: bool,
    pub game_input_method: GameInputMethod,
    pub game_input_delay_ms: u64,
    pub quick_shout_focus_delay_ms: u64,
    pub quick_shouts: Vec<QuickShout>,
}

impl From<&TranslationSettings> for TranslationSettingsView {
    fn from(value: &TranslationSettings) -> Self {
        Self {
            api_url: value.api_url.clone(),
            proxy_url: value.proxy_url.clone(),
            api_key_configured: !value.api_key.trim().is_empty(),
            model: value.model.clone(),
            ocr_language: value.ocr_language.clone(),
            capture_hotkey: value.capture_hotkey.clone(),
            chat_region: value.chat_region,
            incoming_prompt: value.incoming_prompt.clone(),
            outgoing_prompt: value.outgoing_prompt.clone(),
            incoming_translation_display_mode: value.incoming_translation_display_mode,
            translation_hud_position: value.translation_hud_position,
            game_overlay_enabled: value.game_overlay_enabled,
            overlay_chat_key: value.overlay_chat_key.clone(),
            auto_lock_caps: value.auto_lock_caps,
            game_input_method: value.game_input_method,
            game_input_delay_ms: value.game_input_delay_ms,
            quick_shout_focus_delay_ms: value.quick_shout_focus_delay_ms,
            quick_shouts: value.quick_shouts.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslationError {
    MissingApiUrl,
    InvalidApiUrl,
    InvalidProxy,
    MissingModel,
    InvalidRegion,
    InvalidHudPosition,
    Request(String),
    Response(String),
    Storage(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedChatLine {
    pub speaker: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslatedChatLine {
    pub speaker: String,
    pub original_message: String,
    pub translated_message: String,
}

#[derive(Debug, Default)]
pub struct ChatLineTracker {
    generation: Option<u64>,
    visible_lines: Vec<ParsedChatLine>,
}

impl ChatLineTracker {
    pub fn pending_lines(
        &self,
        generation: u64,
        current: &[ParsedChatLine],
    ) -> Vec<ParsedChatLine> {
        if self.generation != Some(generation) || self.visible_lines.is_empty() {
            return current.to_vec();
        }

        let start = new_line_start(&self.visible_lines, current);
        current[start..].to_vec()
    }

    pub fn commit(&mut self, generation: u64, current: &[ParsedChatLine]) {
        self.generation = Some(generation);
        self.visible_lines = current.to_vec();
    }

    pub fn reset(&mut self) {
        self.generation = None;
        self.visible_lines.clear();
    }
}

fn new_line_start(previous: &[ParsedChatLine], current: &[ParsedChatLine]) -> usize {
    if previous.is_empty() {
        return 0;
    }
    if current.is_empty() {
        return 0;
    }

    let mut best_end = 0;
    let mut best_length = 0;
    for current_start in 0..current.len() {
        let maximum = previous.len().min(current.len() - current_start);
        for length in (1..=maximum).rev() {
            let previous_start = previous.len() - length;
            if chat_slices_match(
                &previous[previous_start..],
                &current[current_start..current_start + length],
            ) {
                if length > best_length {
                    best_length = length;
                    best_end = current_start + length;
                }
                break;
            }
        }
    }

    if best_length == 0 { 0 } else { best_end }
}

fn chat_slices_match(previous: &[ParsedChatLine], current: &[ParsedChatLine]) -> bool {
    previous
        .iter()
        .zip(current)
        .all(|(left, right)| chat_lines_match(left, right))
}

fn chat_lines_match(left: &ParsedChatLine, right: &ParsedChatLine) -> bool {
    fuzzy_text_match(&left.speaker, &right.speaker)
        && fuzzy_text_match(&left.message, &right.message)
}

fn fuzzy_text_match(left: &str, right: &str) -> bool {
    let left = comparison_text(left);
    let right = comparison_text(right);
    if left == right {
        return true;
    }
    if left.is_empty() || right.is_empty() {
        return left.is_empty() && right.is_empty();
    }

    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();
    let maximum = left_chars.len().max(right_chars.len());
    if maximum < 8 || left_chars.len().abs_diff(right_chars.len()) > 2 {
        return false;
    }
    levenshtein_distance(&left_chars, &right_chars) <= (maximum / 10).clamp(1, 2)
}

fn comparison_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn levenshtein_distance(left: &[char], right: &[char]) -> usize {
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];
    for (left_index, left_character) in left.iter().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_character) in right.iter().enumerate() {
            let substitution = usize::from(left_character != right_character);
            current[right_index + 1] = (current[right_index] + 1)
                .min(previous[right_index + 1] + 1)
                .min(previous[right_index] + substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

pub fn parse_chat_line(value: &str) -> ParsedChatLine {
    let segments = split_ocr_chat_segments(value);
    if segments.len() == 1 {
        return segments.into_iter().next().unwrap_or(ParsedChatLine {
            speaker: String::new(),
            message: value.trim().to_owned(),
        });
    }
    // Keep the first complete chat segment when OCR glues multiple messages.
    segments
        .into_iter()
        .find(|line| !line.message.trim().is_empty())
        .unwrap_or(ParsedChatLine {
            speaker: String::new(),
            message: value.trim().to_owned(),
        })
}

pub fn expand_ocr_chat_line(value: &str) -> Vec<ParsedChatLine> {
    split_ocr_chat_segments(value)
        .into_iter()
        .filter(|line| !line.message.trim().is_empty())
        .collect()
}

fn split_ocr_chat_segments(value: &str) -> Vec<ParsedChatLine> {
    let normalized = collapse_ocr_noise(value);
    if normalized.is_empty() {
        return Vec::new();
    }

    let chars: Vec<char> = normalized.chars().collect();
    let mut starts = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if let Some(length) = speaker_prefix_length(&chars[index..]) {
            let previous = index.checked_sub(1).map(|value| chars[value]);
            let boundary = match previous {
                None => true,
                Some(character)
                    if character.is_whitespace()
                        || matches!(
                            character,
                            '·' | '|'
                                | '/'
                                | ';'
                                | '；'
                                | ','
                                | '，'
                                | '.'
                                | '!'
                                | '?'
                                | '。'
                                | '！'
                                | '？'
                        ) =>
                {
                    true
                }
                // OCR sometimes glues "...wordName:" without a separator.
                Some(character)
                    if character.is_ascii_alphanumeric() && contains_han_char(chars[index]) =>
                {
                    true
                }
                _ => false,
            };
            if boundary {
                starts.push(index);
                index += length;
                continue;
            }
        }
        index += 1;
    }

    // An OCR colon inside a long message can look like an English speaker
    // label (for example, "Super Earth is a pig: ..."). Keep it in the
    // message so the flattened-line splitter can recover the earlier text.
    if starts.len() == 1 {
        let start = starts[0];
        let prefix: String = chars[..start].iter().collect();
        let label: String = chars[start..]
            .iter()
            .copied()
            .take_while(|character| !matches!(character, ':' | '：'))
            .collect();
        if start > 0
            && prefix.split_whitespace().count() >= 3
            && label.trim().chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            })
        {
            starts.clear();
        }
    }

    if starts.is_empty() {
        let pieces = split_unmarked_message(&normalized);
        if pieces.len() > 1 {
            return pieces
                .into_iter()
                .filter_map(|piece| {
                    let message = clean_message(&piece);
                    (!message.is_empty()).then_some(ParsedChatLine {
                        speaker: String::new(),
                        message,
                    })
                })
                .collect();
        }
        return match parse_single_chat_line(&normalized) {
            Some(parsed) => vec![parsed],
            None => Vec::new(),
        };
    }

    let mut segments = Vec::with_capacity(starts.len());
    for (position, start) in starts.iter().copied().enumerate() {
        let end = starts.get(position + 1).copied().unwrap_or(chars.len());
        let chunk: String = chars[start..end].iter().collect();
        let chunk = chunk
            .trim()
            .trim_matches(|c: char| matches!(c, '·' | '|' | ';' | '；'));
        if chunk.is_empty() {
            continue;
        }
        if let Some(parsed) = parse_single_chat_line(chunk) {
            if !parsed.message.trim().is_empty() {
                segments.extend(expand_unmarked_chat_line(parsed));
            }
        }
    }

    if segments.is_empty() {
        if let Some(parsed) = parse_single_chat_line(&normalized) {
            return expand_unmarked_chat_line(parsed);
        }
        return vec![ParsedChatLine {
            speaker: String::new(),
            message: normalized,
        }];
    }
    segments
}

fn expand_unmarked_chat_line(line: ParsedChatLine) -> Vec<ParsedChatLine> {
    let pieces = split_unmarked_message(&line.message);
    if pieces.len() <= 1 {
        return vec![line];
    }
    pieces
        .into_iter()
        .filter_map(|piece| {
            let message = clean_message(&piece);
            (!message.is_empty()).then_some(ParsedChatLine {
                speaker: line.speaker.clone(),
                message,
            })
        })
        .collect()
}

fn split_unmarked_message(value: &str) -> Vec<String> {
    let normalized = collapse_ocr_noise(value);
    let tokens = ocr_token_ranges(&normalized);
    if tokens.len() < 4 {
        return vec![normalized];
    }

    let mut boundaries = Vec::new();
    let mut segment_start = 0;
    for index in 0..tokens.len() - 1 {
        let (_, token_end) = tokens[index];
        let (next_start, next_end) = tokens[index + 1];
        let token = &normalized[tokens[index].0..token_end];
        let next = &normalized[next_start..next_end];
        let current = normalized[segment_start..token_end].trim();
        let current_word_count = current.split_whitespace().count();
        let next_starts_latin = next
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic());
        let colon_boundary = token
            .chars()
            .any(|character| matches!(character, ':' | '：'));
        let sentence_boundary = token
            .chars()
            .next_back()
            .is_some_and(is_ocr_sentence_punctuation);
        let bro_boundary = next
            .trim_matches(|character: char| !character.is_ascii_alphabetic())
            .eq_ignore_ascii_case("bro")
            && current_word_count >= 4;
        let opener_boundary = (is_likely_chat_opener(next) || bro_boundary)
            && !token
                .chars()
                .next_back()
                .is_some_and(|character| matches!(character, ',' | '，' | ';' | '；'));
        let trailing_term_boundary = index + 1 == tokens.len() - 1
            && current_word_count >= 3
            && is_likely_terminal_chat_word(next);
        let should_split = next_starts_latin
            && ((colon_boundary && current_word_count >= 2)
                || (sentence_boundary && current_word_count >= 1)
                || (opener_boundary && current.chars().count() >= 8 && current_word_count >= 3));
        let should_split = should_split || (next_starts_latin && trailing_term_boundary);
        if should_split {
            boundaries.push(token_end);
            segment_start = token_end;
        }
    }

    // A single long sentence should remain one message. Require multiple
    // independent boundaries before treating a flattened OCR line as chat.
    if boundaries.len() < 2 {
        return vec![normalized];
    }
    let mut output = Vec::with_capacity(boundaries.len() + 1);
    let mut start = 0;
    for end in boundaries
        .into_iter()
        .chain(std::iter::once(normalized.len()))
    {
        let piece = normalized[start..end].trim();
        if !piece.is_empty() {
            output.push(piece.to_owned());
        }
        start = end;
    }
    output
}

fn ocr_token_ranges(value: &str) -> Vec<(usize, usize)> {
    let mut tokens = Vec::new();
    let mut start = None;
    for (index, character) in value.char_indices() {
        if character.is_whitespace() {
            if let Some(token_start) = start.take() {
                tokens.push((token_start, index));
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(token_start) = start {
        tokens.push((token_start, value.len()));
    }
    tokens
}

fn is_likely_chat_opener(value: &str) -> bool {
    let word = value
        .trim_matches(|character: char| !character.is_ascii_alphabetic() && character != '\'')
        .to_ascii_lowercase();
    matches!(
        word.as_str(),
        "let"
            | "lets"
            | "let's"
            | "letl"
            | "help"
            | "nice"
            | "evac"
            | "follow"
            | "wait"
            | "hold"
            | "ready"
            | "need"
            | "thanks"
            | "sorry"
    )
}

fn is_likely_terminal_chat_word(value: &str) -> bool {
    let word = value
        .trim_matches(|character: char| !character.is_ascii_alphabetic())
        .to_ascii_lowercase();
    matches!(
        word.as_str(),
        "charger"
            | "warrior"
            | "hunter"
            | "stalker"
            | "titan"
            | "hulk"
            | "devastator"
            | "overseer"
            | "evac"
            | "reinforce"
    )
}

fn is_ocr_sentence_punctuation(character: char) -> bool {
    matches!(character, '.' | '!' | '?' | '。' | '！' | '？')
}

fn parse_single_chat_line(value: &str) -> Option<ParsedChatLine> {
    let normalized = collapse_ocr_noise(value);
    if normalized.is_empty() {
        return None;
    }
    for delimiter in [':', '：'] {
        if let Some((speaker, message)) = normalized.split_once(delimiter) {
            let speaker = clean_speaker(speaker);
            let message = clean_message(message);
            if is_plausible_speaker(&speaker) {
                if message.is_empty() {
                    // Bare "Name:" OCR crumbs should not become fake chat lines.
                    return None;
                }
                return Some(ParsedChatLine { speaker, message });
            }
        }
    }
    let message = clean_message(&normalized);
    if message.is_empty() || is_speaker_only_crumb(&message) {
        None
    } else {
        Some(ParsedChatLine {
            speaker: String::new(),
            message,
        })
    }
}

fn is_speaker_only_crumb(value: &str) -> bool {
    let trimmed = value
        .trim()
        .trim_matches(|c: char| matches!(c, ':' | '：' | '·' | '|' | ';' | '；' | ',' | '，'));
    // Only Chinese name crumbs; short English words like "tasks." are real messages.
    !trimmed.is_empty()
        && !trimmed.contains(' ')
        && trimmed.chars().all(contains_han_char)
        && is_plausible_speaker(trimmed)
}

fn speaker_prefix_length(chars: &[char]) -> Option<usize> {
    if chars.is_empty() {
        return None;
    }
    let mut length = 0;
    let mut has_han = false;
    let mut has_latin = false;
    while length < chars.len() && length < 24 {
        let character = chars[length];
        if character.is_whitespace()
            || matches!(
                character,
                ':' | '：' | '·' | '|' | '/' | ';' | '；' | ',' | '，'
            )
        {
            break;
        }
        if contains_han_char(character) {
            // Reject mixed-script OCR glue like "warrior牡蛎:".
            if has_latin {
                return None;
            }
            has_han = true;
        } else if character.is_ascii_alphanumeric() {
            if has_han {
                return None;
            }
            has_latin = true;
        } else if !matches!(
            character,
            '_' | '-' | '.' | '[' | ']' | '(' | ')' | '（' | '）'
        ) {
            break;
        }
        length += 1;
    }
    if length == 0 || length > 20 || !(has_han || has_latin) {
        return None;
    }
    let mut cursor = length;
    while cursor < chars.len() && chars[cursor].is_whitespace() {
        cursor += 1;
    }
    if cursor >= chars.len() || !matches!(chars[cursor], ':' | '：') {
        return None;
    }
    cursor += 1;
    while cursor < chars.len() && chars[cursor].is_whitespace() {
        cursor += 1;
    }
    // Require some message body after the colon so bare "Name:" fragments are ignored.
    if cursor >= chars.len() {
        return None;
    }
    Some(cursor)
}

fn collapse_ocr_noise(value: &str) -> String {
    let characters: Vec<char> = value.chars().collect();
    let mut output = String::with_capacity(value.len());
    let mut previous_space = false;
    for (index, character) in characters.iter().copied().enumerate() {
        if matches!(character, '《' | '》')
            && characters[..index]
                .iter()
                .rev()
                .find(|candidate| !candidate.is_whitespace())
                .is_some_and(|candidate| candidate.is_ascii_alphabetic())
            && characters[index + 1..]
                .iter()
                .find(|candidate| !candidate.is_whitespace())
                .is_some_and(|candidate| candidate.is_ascii_alphabetic())
        {
            while output.ends_with(' ') {
                output.pop();
            }
            output.push('\'');
            previous_space = false;
            continue;
        }
        if character == '\u{00a0}' || character.is_whitespace() {
            if output.ends_with('\'') {
                continue;
            }
            if !previous_space && !output.is_empty() {
                output.push(' ');
                previous_space = true;
            }
            continue;
        }
        if matches!(character, '·' | '•' | '●') {
            // OCR often inserts interpuncts between glued chat lines.
            if !previous_space && !output.is_empty() {
                output.push(' ');
                previous_space = true;
            }
            continue;
        }
        let output_character = match character {
            '，' => ',',
            '。' => '.',
            '：' => ':',
            '；' => ';',
            '！' => '!',
            '？' => '?',
            _ => character,
        };
        if is_ocr_closing_punctuation(output_character) {
            while output.ends_with(' ') {
                output.pop();
            }
        }
        previous_space = false;
        output.push(output_character);
    }
    output
        .trim()
        .replace("B ro", "Bro")
        .replace("Letl s", "Let's")
        .replace("letl s", "let's")
        .replace("dO", "do")
        .replace("Let'S", "Let's")
        .replace("let'S", "let's")
}

fn is_ocr_closing_punctuation(character: char) -> bool {
    matches!(
        character,
        ',' | '.'
            | ':'
            | ';'
            | '!'
            | '?'
            | ')'
            | ']'
            | '}'
            | '，'
            | '。'
            | '：'
            | '；'
            | '！'
            | '？'
            | '）'
            | '】'
            | '》'
    )
}

fn clean_speaker(value: &str) -> String {
    value
        .trim()
        .trim_matches(|c: char| {
            matches!(
                c,
                '·' | '|' | '/' | ';' | '；' | ',' | '，' | '"' | '\'' | '“' | '”'
            )
        })
        .to_owned()
}

fn clean_message(value: &str) -> String {
    let mut message = value.trim().to_owned();
    loop {
        let trimmed = message.trim_end_matches(|c: char| {
            matches!(c, '·' | '|' | ';' | '；' | ',' | '，' | ':' | '：' | ' ')
        });
        if trimmed == message {
            break;
        }
        message = trimmed.to_owned();
    }

    // Drop a trailing Chinese speaker crumb left by OCR, e.g. "warrior 牡蛎".
    if let Some((head, tail)) = message.rsplit_once(' ') {
        let tail = tail.trim();
        if !tail.is_empty()
            && tail.chars().all(contains_han_char)
            && is_plausible_speaker(tail)
            && !tail.contains(' ')
        {
            message = head.trim_end().to_owned();
        }
    }
    message.trim().to_owned()
}

fn is_plausible_speaker(value: &str) -> bool {
    let count = value.chars().count();
    if value.is_empty() || count > 20 {
        return false;
    }
    if value.chars().any(|c| matches!(c, ':' | '：')) {
        return false;
    }
    let meaningful = value
        .chars()
        .filter(|c| c.is_alphanumeric() || contains_han_char(*c))
        .count();
    meaningful > 0 && meaningful * 2 >= count
}

fn contains_han_char(character: char) -> bool {
    matches!(
        character as u32,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF
    )
}

#[cfg(windows)]
pub fn merge_ocr_chat_lines(
    primary: &[crate::platform::windows::capture::PositionedOcrLine],
    speaker_source: &[crate::platform::windows::capture::PositionedOcrLine],
) -> Vec<ParsedChatLine> {
    primary
        .iter()
        .flat_map(|line| {
            let mut segments = expand_ocr_chat_line(&line.text);
            if segments.is_empty() {
                return segments;
            }
            let best_speaker = speaker_source
                .iter()
                .filter(|candidate| {
                    let tolerance = line.height.max(candidate.height).max(12.0) * 0.7;
                    (candidate.top - line.top).abs() <= tolerance
                })
                .flat_map(|candidate| expand_ocr_chat_line(&candidate.text))
                .find(|candidate| !candidate.speaker.is_empty());
            if let Some(candidate) = best_speaker {
                for segment in &mut segments {
                    if segment.speaker.is_empty() {
                        segment.speaker = candidate.speaker.clone();
                    }
                }
            }
            segments
        })
        .filter(|line| !line.message.trim().is_empty())
        .collect()
}

#[cfg(windows)]
pub fn merge_bilingual_ocr_lines(
    chinese: &[crate::platform::windows::capture::PositionedOcrLine],
    english: &[crate::platform::windows::capture::PositionedOcrLine],
) -> Vec<ParsedChatLine> {
    let mut paired_english = vec![false; english.len()];
    let mut merged = Vec::with_capacity(chinese.len().max(english.len()));

    for chinese_line in chinese {
        let best_match = english
            .iter()
            .enumerate()
            .filter(|(index, candidate)| {
                if paired_english[*index] {
                    return false;
                }
                let tolerance = chinese_line.height.max(candidate.height).max(12.0) * 0.75;
                (candidate.top - chinese_line.top).abs() <= tolerance
            })
            .min_by(|(_, left), (_, right)| {
                (left.top - chinese_line.top)
                    .abs()
                    .total_cmp(&(right.top - chinese_line.top).abs())
            });

        if let Some((index, english_line)) = best_match {
            paired_english[index] = true;
            let top = chinese_line.top.min(english_line.top);
            let height = chinese_line.height.max(english_line.height);
            let expanded = expand_preferred_ocr_lines(&chinese_line.text, &english_line.text);
            let fallback_speaker = expand_ocr_chat_line(&chinese_line.text)
                .into_iter()
                .find(|line| !line.speaker.is_empty())
                .map(|line| line.speaker)
                .unwrap_or_default();
            if expanded.is_empty() {
                merged.push((
                    top,
                    height,
                    choose_bilingual_chat_line(&chinese_line.text, &english_line.text),
                ));
            } else {
                for mut parsed in expanded {
                    if parsed.speaker.is_empty() && !fallback_speaker.is_empty() {
                        parsed.speaker = fallback_speaker.clone();
                    }
                    merged.push((top, height, parsed));
                }
            }
        } else {
            for parsed in expand_ocr_chat_line(&chinese_line.text) {
                merged.push((chinese_line.top, chinese_line.height, parsed));
            }
        }
    }

    merged.extend(
        english
            .iter()
            .enumerate()
            .filter(|(index, _)| !paired_english[*index])
            .flat_map(|(_, line)| {
                expand_ocr_chat_line(&line.text)
                    .into_iter()
                    .map(move |parsed| (line.top, line.height, parsed))
            }),
    );
    merged.sort_by(|(left, _, _), (right, _, _)| left.total_cmp(right));
    merge_wrapped_chat_lines(merged)
}

#[cfg(windows)]
fn merge_wrapped_chat_lines(lines: Vec<(f32, f32, ParsedChatLine)>) -> Vec<ParsedChatLine> {
    let mut output: Vec<(f32, f32, ParsedChatLine)> = Vec::with_capacity(lines.len());
    for (top, height, mut line) in lines {
        if line.message.trim().is_empty() {
            continue;
        }
        let mut inherited_speaker = None;
        if line.speaker.is_empty() {
            if let Some((previous_top, previous_height, previous)) = output.last_mut() {
                inherited_speaker =
                    (!previous.speaker.is_empty()).then(|| previous.speaker.clone());
                let previous_bottom = *previous_top + *previous_height;
                let gap = top - previous_bottom;
                let wrap_tolerance = previous_height.max(height).max(12.0) * 1.8;
                let previous_word_count = previous.message.split_whitespace().count();
                let previous_is_long =
                    previous_word_count >= 6 || previous.message.chars().count() >= 28;
                let continuation_starts_lowercase = line
                    .message
                    .trim_start()
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_lowercase());
                let looks_like_wrap = gap <= wrap_tolerance
                    && previous_is_long
                    && continuation_starts_lowercase
                    && !previous
                        .message
                        .chars()
                        .next_back()
                        .is_some_and(is_ocr_sentence_punctuation);
                if !previous.speaker.is_empty() && looks_like_wrap {
                    previous.message = join_wrapped_message(&previous.message, &line.message);
                    *previous_height = (top + height - *previous_top).max(*previous_height);
                    continue;
                }
            }
        }
        if line.speaker.is_empty() {
            if let Some(speaker) = inherited_speaker {
                line.speaker = speaker;
            }
        }
        output.push((top, height, line));
    }
    output.into_iter().map(|(_, _, line)| line).collect()
}

#[cfg(windows)]
fn join_wrapped_message(first: &str, continuation: &str) -> String {
    let first = first.trim_end();
    let continuation = continuation.trim_start();
    if first.ends_with('-') {
        format!("{}{}", first.trim_end_matches('-'), continuation)
    } else {
        format!("{first} {continuation}")
    }
}

#[cfg(windows)]
fn expand_preferred_ocr_lines(chinese: &str, english: &str) -> Vec<ParsedChatLine> {
    let chinese_lines = expand_ocr_chat_line(chinese);
    let english_lines = expand_ocr_chat_line(english);
    if chinese_lines.len() > 1 {
        return chinese_lines;
    }
    if english_lines.len() > 1 {
        return english_lines;
    }
    Vec::new()
}

#[cfg(windows)]
fn choose_bilingual_chat_line(chinese: &str, english: &str) -> ParsedChatLine {
    // Prefer the best single segment from each OCR engine, then pick fields.
    let chinese = expand_ocr_chat_line(chinese)
        .into_iter()
        .next()
        .unwrap_or_else(|| parse_chat_line(chinese));
    let english = expand_ocr_chat_line(english)
        .into_iter()
        .next()
        .unwrap_or_else(|| parse_chat_line(english));
    let message = if contains_han(&chinese.message) {
        chinese.message.clone()
    } else if latin_text_score(&english.message) > 0
        && latin_text_score(&english.message) >= latin_text_score(&chinese.message)
    {
        english.message.clone()
    } else if !chinese.message.is_empty() {
        chinese.message.clone()
    } else {
        english.message.clone()
    };
    let speaker = if contains_han(&chinese.speaker) {
        chinese.speaker
    } else if !english.speaker.is_empty() {
        english.speaker
    } else {
        chinese.speaker
    };
    ParsedChatLine { speaker, message }
}

#[cfg(windows)]
fn contains_han(value: &str) -> bool {
    value.chars().any(|character| {
        matches!(
            character as u32,
            0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF
        )
    })
}

#[cfg(windows)]
fn latin_text_score(value: &str) -> usize {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .count()
}

#[cfg(windows)]
pub fn prefer_latin_ocr<'a>(
    configured: &'a crate::platform::windows::capture::OcrReading,
    english: &'a crate::platform::windows::capture::OcrReading,
) -> &'a crate::platform::windows::capture::OcrReading {
    let configured_score = latin_ocr_score(configured);
    let english_score = latin_ocr_score(english);
    if english_score > configured_score.saturating_add(4) {
        english
    } else {
        configured
    }
}

#[cfg(windows)]
fn latin_ocr_score(reading: &crate::platform::windows::capture::OcrReading) -> usize {
    reading
        .lines
        .iter()
        .flat_map(|line| line.text.chars())
        .filter(|character| character.is_ascii_alphabetic())
        .count()
}

pub async fn translate_chat_lines(
    settings: &TranslationSettings,
    lines: &[ParsedChatLine],
) -> Result<Vec<TranslatedChatLine>, TranslationError> {
    if lines.is_empty() {
        return Err(TranslationError::Response(
            "没有可翻译的聊天消息".to_owned(),
        ));
    }

    // Realtime-friendly path: one short request per new chat line.
    // This keeps reasoning models from burning the whole token budget on thinking.
    let mut translated_lines = Vec::with_capacity(lines.len());
    for line in lines {
        let translated_message = translate_with_token_budget(
            settings,
            configured_prompt(&settings.incoming_prompt, REFERENCE_INCOMING_PROMPT),
            line.message.trim(),
            64,
            false,
        )
        .await?
        .trim()
        .trim_matches('"')
        .to_owned();
        if translated_message.is_empty() {
            return Err(TranslationError::Response(
                "聊天翻译返回了空结果".to_owned(),
            ));
        }
        translated_lines.push(TranslatedChatLine {
            speaker: line.speaker.clone(),
            original_message: line.message.clone(),
            translated_message,
        });
    }
    Ok(translated_lines)
}

const TEST_SYSTEM_PROMPT: &str = "Reply with exactly OK";

fn configured_prompt<'a>(configured: &'a str, fallback: &'a str) -> &'a str {
    if configured.trim().is_empty() {
        fallback
    } else {
        configured.trim()
    }
}

#[cfg(test)]
fn extract_json_array(value: &str) -> Option<&str> {
    let start = value.find('[')?;
    let end = value.rfind(']')?;
    (end >= start).then_some(&value[start..=end])
}

pub fn normalize_chat_completions_url(raw: &str) -> Result<String, TranslationError> {
    let trimmed = raw.trim().trim_end_matches('/');
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err(if trimmed.is_empty() {
            TranslationError::MissingApiUrl
        } else {
            TranslationError::InvalidApiUrl
        });
    }
    if trimmed.ends_with("/chat/completions") {
        Ok(trimmed.to_owned())
    } else {
        Ok(format!("{trimmed}/chat/completions"))
    }
}

pub fn validate_settings(settings: &TranslationSettings) -> Result<(), TranslationError> {
    normalize_chat_completions_url(&settings.api_url)?;
    if settings.model.trim().is_empty() {
        return Err(TranslationError::MissingModel);
    }
    if !settings.proxy_url.trim().is_empty() {
        reqwest::Proxy::all(settings.proxy_url.trim())
            .map_err(|_| TranslationError::InvalidProxy)?;
    }
    if let Some(region) = settings.chat_region {
        region.validate()?;
    }
    if let Some(position) = settings.translation_hud_position {
        position.validate()?;
    }
    Ok(())
}

pub fn load_settings(path: &Path) -> Result<TranslationSettings, TranslationError> {
    if !path.exists() {
        return Ok(TranslationSettings::default());
    }
    let raw = std::fs::read_to_string(path)
        .map_err(|error| TranslationError::Storage(error.to_string()))?;
    let mut settings: TranslationSettings =
        serde_json::from_str(&raw).map_err(|error| TranslationError::Storage(error.to_string()))?;
    refresh_stale_default_prompts(&mut settings);
    settings.game_input_method = normalize_game_input_method(settings.game_input_method);
    settings.quick_shout_focus_delay_ms =
        clamp_quick_shout_focus_delay_ms(settings.quick_shout_focus_delay_ms);
    settings.game_input_delay_ms = clamp_game_input_delay_ms(settings.game_input_delay_ms);
    Ok(settings)
}

fn refresh_stale_default_prompts(settings: &mut TranslationSettings) {
    if is_stale_default_prompt(&settings.incoming_prompt) {
        settings.incoming_prompt = REFERENCE_INCOMING_PROMPT.to_owned();
    }
    if is_stale_default_prompt(&settings.outgoing_prompt)
        || settings
            .outgoing_prompt
            .contains(LEGACY_OUTGOING_GLOSSARY_RULE)
    {
        settings.outgoing_prompt = REFERENCE_OUTGOING_PROMPT.to_owned();
    }
}

fn is_stale_default_prompt(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return true;
    }
    if trimmed == REFERENCE_INCOMING_PROMPT || trimmed == REFERENCE_OUTGOING_PROMPT {
        return false;
    }
    if trimmed == DEFAULT_INCOMING_PROMPT || trimmed == DEFAULT_OUTGOING_PROMPT {
        return true;
    }
    // Upgrade old built-ins only. Long custom prompts stay untouched.
    trimmed.starts_with("You are a HELLDIVERS 2 in-game chat translator.")
        || trimmed.starts_with("HD2跨服聊天 EN→简中。")
        || trimmed.starts_with("HD2跨服聊天 中→EN。")
        || (trimmed.starts_with("你是《绝地潜兵2》跨服聊天")
            && trimmed != REFERENCE_INCOMING_PROMPT
            && trimmed != REFERENCE_OUTGOING_PROMPT
            && trimmed.chars().count() < DEFAULT_INCOMING_PROMPT.chars().count().saturating_sub(40))
}

pub fn save_settings(path: &Path, settings: &TranslationSettings) -> Result<(), TranslationError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| TranslationError::Storage(error.to_string()))?;
    }
    let encoded = serde_json::to_vec_pretty(settings)
        .map_err(|error| TranslationError::Storage(error.to_string()))?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, encoded)
        .map_err(|error| TranslationError::Storage(error.to_string()))?;
    replace_file(&temporary, path)
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> Result<(), TranslationError> {
    use std::{iter::once, os::windows::ffi::OsStrExt};
    use windows::{
        Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        },
        core::PCWSTR,
    };

    let source_wide: Vec<u16> = source.as_os_str().encode_wide().chain(once(0)).collect();
    let destination_wide: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(once(0))
        .collect();
    unsafe {
        MoveFileExW(
            PCWSTR(source_wide.as_ptr()),
            PCWSTR(destination_wide.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|error| TranslationError::Storage(error.to_string()))
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> Result<(), TranslationError> {
    if destination.exists() {
        std::fs::remove_file(destination)
            .map_err(|error| TranslationError::Storage(error.to_string()))?;
    }
    std::fs::rename(source, destination)
        .map_err(|error| TranslationError::Storage(error.to_string()))
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    temperature: f32,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    enable_thinking: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<serde_json::Value>,
    messages: [ChatMessage<'a>; 2],
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    #[serde(default)]
    finish_reason: Option<String>,
    message: ChatResponseMessage,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
}

pub async fn translate(
    settings: &TranslationSettings,
    system_prompt: &str,
    text: &str,
) -> Result<String, TranslationError> {
    translate_with_token_budget(settings, system_prompt, text, 256, false).await
}

pub async fn translate_outgoing_message(
    settings: &TranslationSettings,
    text: &str,
) -> Result<String, TranslationError> {
    translate_with_token_budget(
        settings,
        configured_prompt(&settings.outgoing_prompt, REFERENCE_OUTGOING_PROMPT),
        text,
        64,
        false,
    )
    .await
}

pub async fn translate_connection_test(
    settings: &TranslationSettings,
) -> Result<String, TranslationError> {
    translate_with_token_budget(settings, TEST_SYSTEM_PROMPT, "test", 16, false).await
}

async fn translate_with_token_budget(
    settings: &TranslationSettings,
    system_prompt: &str,
    text: &str,
    max_tokens: u32,
    allow_budget_retry: bool,
) -> Result<String, TranslationError> {
    validate_settings(settings)?;
    let url = normalize_chat_completions_url(&settings.api_url)?;
    let proxy_mode = if settings.proxy_url.trim().is_empty() {
        "direct".to_owned()
    } else {
        format!("explicit:{}", settings.proxy_url.trim())
    };
    let started_at = std::time::Instant::now();
    let mut client_builder = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(API_CONNECT_TIMEOUT_SECS))
        .timeout(std::time::Duration::from_secs(API_REQUEST_TIMEOUT_SECS))
        .http1_only();
    if settings.proxy_url.trim().is_empty() {
        client_builder = client_builder.no_proxy();
    } else {
        let proxy = reqwest::Proxy::all(settings.proxy_url.trim())
            .map_err(|_| TranslationError::InvalidProxy)?;
        client_builder = client_builder.proxy(proxy);
    }
    let client = client_builder
        .build()
        .map_err(|error| TranslationError::Request(error.to_string()))?;

    let mut current_max_tokens = max_tokens.max(16);
    let attempts = if allow_budget_retry { 2 } else { 1 };
    for output_attempt in 1..=attempts {
        #[cfg(debug_assertions)]
        eprintln!(
            "[hd2cn][translation] stage=request_start endpoint={} proxy={} model={} input_chars={} output_attempt={} connect_timeout_s={} request_timeout_s={} max_tokens={} thinking=false",
            url,
            proxy_mode,
            settings.model.trim(),
            text.chars().count(),
            output_attempt,
            API_CONNECT_TIMEOUT_SECS,
            API_REQUEST_TIMEOUT_SECS,
            current_max_tokens
        );
        let mut include_thinking_switch = true;
        let (_status, _content_type, body) = loop {
            let request_body = ChatRequest {
                model: settings.model.trim(),
                temperature: 0.0,
                max_tokens: current_max_tokens,
                enable_thinking: include_thinking_switch.then_some(false),
                thinking: include_thinking_switch
                    .then(|| serde_json::json!({ "type": "disabled" })),
                messages: [
                    ChatMessage {
                        role: "system",
                        content: system_prompt,
                    },
                    ChatMessage {
                        role: "user",
                        content: text,
                    },
                ],
            };
            let (status, content_type, body) = send_translation_request(
                &client,
                &url,
                &request_body,
                settings.api_key.trim(),
                settings.proxy_url.trim().is_empty(),
                &proxy_mode,
                started_at,
            )
            .await?;
            if !status.is_success() {
                let summary = response_body_summary(&body, 300);
                let lower = summary.to_ascii_lowercase();
                if include_thinking_switch
                    && (status.as_u16() == 400 || status.as_u16() == 422)
                    && (lower.contains("thinking")
                        || lower.contains("enable_thinking")
                        || lower.contains("unknown field")
                        || lower.contains("unrecognized"))
                {
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "[hd2cn][translation] stage=thinking_switch_fallback status={} summary={}",
                        status.as_u16(),
                        summary
                    );
                    include_thinking_switch = false;
                    continue;
                }
                return Err(TranslationError::Response(format!(
                    "HTTP {}: {}",
                    status.as_u16(),
                    summary
                )));
            }
            break (status, content_type, body);
        };
        let parsed: ChatResponse = serde_json::from_slice(&body).map_err(|error| {
            let summary = response_body_summary(&body, 300);
            #[cfg(debug_assertions)]
            eprintln!(
                "[hd2cn][translation] stage=response_json_error elapsed_ms={} status={} content_type={} body_bytes={} error={error:?}",
                started_at.elapsed().as_millis(),
                _status.as_u16(),
                _content_type,
                body.len()
            );
            TranslationError::Response(format!(
                "翻译响应不是有效的 OpenAI 兼容 JSON：{error}；正文摘要：{summary}"
            ))
        })?;
        let choice =
            parsed.choices.into_iter().next().ok_or_else(|| {
                TranslationError::Response("翻译接口返回的 choices 为空".to_owned())
            })?;
        if let Some(content) = choice
            .message
            .content
            .map(|content| content.trim().to_owned())
            .filter(|content| !content.is_empty())
        {
            return Ok(content);
        }
        let reasoning_chars = choice
            .message
            .reasoning_content
            .as_deref()
            .map(str::chars)
            .map(Iterator::count)
            .unwrap_or(0);
        if allow_budget_retry
            && choice.finish_reason.as_deref() == Some("length")
            && output_attempt == 1
        {
            #[cfg(debug_assertions)]
            eprintln!(
                "[hd2cn][translation] stage=output_budget_retry elapsed_ms={} reasoning_chars={} previous_max_tokens={} next_max_tokens={}",
                started_at.elapsed().as_millis(),
                reasoning_chars,
                current_max_tokens,
                current_max_tokens.saturating_mul(2).min(512)
            );
            current_max_tokens = current_max_tokens.saturating_mul(2).min(512);
            continue;
        }
        return Err(TranslationError::Response(format!(
            "翻译模型未返回最终 content（finish_reason={}，reasoning_chars={}，max_tokens={}）。当前模型可能强制开启思考模式，请改用非推理/Flash 模型以获得实时翻译。",
            choice.finish_reason.as_deref().unwrap_or("unknown"),
            reasoning_chars,
            current_max_tokens
        )));
    }
    Err(TranslationError::Response(
        "翻译模型在扩大输出额度后仍未返回最终结果".to_owned(),
    ))
}

async fn send_translation_request(
    client: &reqwest::Client,
    url: &str,
    request_body: &ChatRequest<'_>,
    api_key: &str,
    direct_connection: bool,
    _proxy_mode: &str,
    _started_at: std::time::Instant,
) -> Result<(reqwest::StatusCode, String, Vec<u8>), TranslationError> {
    let mut completed_response = None;
    for attempt in 1..=2 {
        let mut request = client
            .post(url)
            .header(reqwest::header::ACCEPT_ENCODING, "identity")
            .json(request_body);
        if !api_key.is_empty() {
            request = request.bearer_auth(api_key);
        }
        let response = request.send().await.map_err(|error| {
            #[cfg(debug_assertions)]
            eprintln!(
                "[hd2cn][translation] stage=request_error elapsed_ms={} proxy={} timeout={} connect={} error={error:?}",
                _started_at.elapsed().as_millis(),
                _proxy_mode,
                error.is_timeout(),
                error.is_connect()
            );
            request_error(error, direct_connection)
        })?;
        let status = response.status();
        let content_encoding = response
            .headers()
            .get(reqwest::header::CONTENT_ENCODING)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("identity")
            .to_owned();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("unknown")
            .to_owned();
        let _content_length = response.content_length();
        #[cfg(debug_assertions)]
        eprintln!(
            "[hd2cn][translation] stage=response_headers elapsed_ms={} proxy={} attempt={} status={} content_encoding={} content_type={} content_length={:?}",
            _started_at.elapsed().as_millis(),
            _proxy_mode,
            attempt,
            status.as_u16(),
            content_encoding,
            content_type,
            _content_length
        );
        match response.bytes().await {
            Ok(body) => {
                completed_response = Some((status, content_type, body.to_vec()));
                break;
            }
            Err(_error) if attempt == 1 => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "[hd2cn][translation] stage=response_body_retry elapsed_ms={} status={} content_encoding={} content_type={} error={_error:?}",
                    _started_at.elapsed().as_millis(),
                    status.as_u16(),
                    content_encoding,
                    content_type
                );
            }
            Err(error) => {
                return Err(TranslationError::Response(format!(
                    "读取翻译响应正文失败（HTTP {}，Content-Encoding: {}，重试 1 次后仍失败）：{}",
                    status.as_u16(),
                    content_encoding,
                    error
                )));
            }
        }
    }
    completed_response.ok_or_else(|| {
        TranslationError::Response("翻译响应正文读取失败，未获得可用结果".to_owned())
    })
}

fn request_error(error: reqwest::Error, direct_connection: bool) -> TranslationError {
    let detail = if error.is_timeout() && error.is_connect() {
        format!("连接 API 超时（连接阶段超过 {API_CONNECT_TIMEOUT_SECS} 秒）")
    } else if error.is_timeout() {
        format!("API 已连接，但模型响应超过 {API_REQUEST_TIMEOUT_SECS} 秒")
    } else if error.is_connect() {
        format!("无法连接 API：{error}")
    } else {
        error.to_string()
    };
    let hint = if direct_connection && error.is_connect() {
        "；当前为直连，请在设置中填写可用的网络代理"
    } else {
        ""
    };
    TranslationError::Request(format!("{detail}{hint}"))
}

fn response_body_summary(body: &[u8], maximum_chars: usize) -> String {
    String::from_utf8_lossy(body)
        .chars()
        .filter(|character| !character.is_control() || character.is_whitespace())
        .take(maximum_chars)
        .collect::<String>()
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_openai_compatible_urls() {
        assert_eq!(
            normalize_chat_completions_url("https://example.com/v1/").unwrap(),
            "https://example.com/v1/chat/completions"
        );
        assert_eq!(
            normalize_chat_completions_url("http://localhost:11434/v1/chat/completions/").unwrap(),
            "http://localhost:11434/v1/chat/completions"
        );
        assert_eq!(
            normalize_chat_completions_url("ftp://example.com"),
            Err(TranslationError::InvalidApiUrl)
        );
    }

    #[test]
    fn validates_normalized_regions() {
        assert!(
            NormalizedRegion {
                x: 0.1,
                y: 0.2,
                width: 0.4,
                height: 0.3
            }
            .validate()
            .is_ok()
        );
        assert_eq!(
            NormalizedRegion {
                x: 0.8,
                y: 0.2,
                width: 0.4,
                height: 0.3
            }
            .validate(),
            Err(TranslationError::InvalidRegion)
        );
    }

    #[test]
    fn settings_view_never_contains_the_api_key() {
        let settings = TranslationSettings {
            api_key: "secret".to_owned(),
            ..TranslationSettings::default()
        };
        let serialized = serde_json::to_string(&TranslationSettingsView::from(&settings)).unwrap();
        assert!(!serialized.contains("secret"));
        assert!(serialized.contains("apiKeyConfigured"));
    }

    #[test]
    fn quick_shout_focus_delay_is_clamped_to_the_supported_range() {
        assert_eq!(clamp_quick_shout_focus_delay_ms(0), 300);
        assert_eq!(clamp_quick_shout_focus_delay_ms(500), 500);
        assert_eq!(clamp_quick_shout_focus_delay_ms(5_000), 1_200);
    }

    #[test]
    fn game_input_delay_is_clamped_to_the_supported_range() {
        assert_eq!(clamp_game_input_delay_ms(0), MIN_GAME_INPUT_DELAY_MS);
        assert_eq!(clamp_game_input_delay_ms(15), 15);
        assert_eq!(clamp_game_input_delay_ms(100), MAX_GAME_INPUT_DELAY_MS);
    }

    #[test]
    fn reference_prompts_include_directional_terms_and_output_rules() {
        assert!(REFERENCE_INCOMING_PROMPT.contains("predator stalker=花蚊子"));
        assert!(REFERENCE_INCOMING_PROMPT.contains("术语翻译前先查询下方核心词库"));
        assert!(REFERENCE_INCOMING_PROMPT.contains("仍无法确定时保留原词"));
        assert!(REFERENCE_INCOMING_PROMPT.contains("只输出最终中文译文"));
        assert!(REFERENCE_OUTGOING_PROMPT.contains("轮椅炮/AT炮=AT emplacement"));
        assert!(REFERENCE_OUTGOING_PROMPT.contains("词库只是参考资料，不是强制替换表"));
        assert!(REFERENCE_OUTGOING_PROMPT.contains("只有输入明确指向《绝地潜兵2》的专用术语"));
        assert!(REFERENCE_OUTGOING_PROMPT.contains("不是专用术语，不得固定替换"));
        assert!(REFERENCE_OUTGOING_PROMPT.contains("必须保留否定和动作关系"));
        assert!(REFERENCE_OUTGOING_PROMPT.contains("普通中文按整句语义自然翻译"));
        assert!(!REFERENCE_OUTGOING_PROMPT.contains(LEGACY_OUTGOING_GLOSSARY_RULE));
        assert!(REFERENCE_OUTGOING_PROMPT.contains("只输出最终英文"));
    }

    #[test]
    fn stale_builtins_refresh_without_overwriting_custom_prompts() {
        let mut settings = TranslationSettings {
            incoming_prompt: DEFAULT_INCOMING_PROMPT.to_owned(),
            outgoing_prompt: DEFAULT_OUTGOING_PROMPT.to_owned(),
            ..TranslationSettings::default()
        };
        refresh_stale_default_prompts(&mut settings);
        assert_eq!(settings.incoming_prompt, REFERENCE_INCOMING_PROMPT);
        assert_eq!(settings.outgoing_prompt, REFERENCE_OUTGOING_PROMPT);

        settings.incoming_prompt = "我的自定义英译中提示词".to_owned();
        settings.outgoing_prompt = format!("旧版中译英提示词：{LEGACY_OUTGOING_GLOSSARY_RULE}");
        refresh_stale_default_prompts(&mut settings);
        assert_eq!(settings.incoming_prompt, "我的自定义英译中提示词");
        assert_eq!(settings.outgoing_prompt, REFERENCE_OUTGOING_PROMPT);

        settings.incoming_prompt = "我的自定义英译中提示词".to_owned();
        settings.outgoing_prompt = "my custom outgoing prompt".to_owned();
        refresh_stale_default_prompts(&mut settings);
        assert_eq!(settings.incoming_prompt, "我的自定义英译中提示词");
        assert_eq!(settings.outgoing_prompt, "my custom outgoing prompt");
    }

    #[test]
    fn legacy_settings_receive_translation_and_quick_shout_defaults() {
        let settings: TranslationSettings = serde_json::from_str(
            r#"{
                "apiUrl": "https://example.com/v1",
                "proxyUrl": "",
                "apiKey": "secret",
                "model": "fast-model",
                "ocrLanguage": "auto",
                "captureHotkey": "CommandOrControl+Shift+T",
                "chatRegion": null
            }"#,
        )
        .unwrap();

        assert_eq!(settings.incoming_prompt, REFERENCE_INCOMING_PROMPT);
        assert_eq!(settings.outgoing_prompt, REFERENCE_OUTGOING_PROMPT);
        assert!(settings.game_overlay_enabled);
        assert_eq!(settings.overlay_chat_key, DEFAULT_OVERLAY_CHAT_KEY);
        assert!(settings.auto_lock_caps);
        assert_eq!(
            settings.game_input_method,
            GameInputMethod::UnicodeSendInput
        );
        assert_eq!(settings.game_input_delay_ms, DEFAULT_GAME_INPUT_DELAY_MS);
        assert_eq!(
            settings.incoming_translation_display_mode,
            IncomingTranslationDisplayMode::ChatTranslationPage
        );
        assert_eq!(settings.translation_hud_position, None);
        assert_eq!(
            settings.quick_shout_focus_delay_ms,
            DEFAULT_QUICK_SHOUT_FOCUS_DELAY_MS
        );
        assert_eq!(settings.quick_shouts.len(), 8);
        assert_eq!(settings.quick_shouts[0].message, "follow me");
    }

    #[test]
    fn legacy_gbk_choice_is_migrated_to_unicode() {
        let settings: TranslationSettings =
            serde_json::from_str(r#"{ "gameInputMethod": "gbkAltCode" }"#).unwrap();
        let mut settings = settings;
        settings.game_input_method = normalize_game_input_method(settings.game_input_method);

        assert_eq!(
            settings.game_input_method,
            GameInputMethod::UnicodeSendInput
        );
        assert_eq!(settings.game_input_delay_ms, DEFAULT_GAME_INPUT_DELAY_MS);
    }

    #[test]
    fn parses_speaker_without_translating_it() {
        assert_eq!(
            parse_chat_line("牡蛎: Super Earth is just a pig."),
            ParsedChatLine {
                speaker: "牡蛎".to_owned(),
                message: "Super Earth is just a pig.".to_owned(),
            }
        );
    }

    #[test]
    fn splits_glued_ocr_chat_messages_with_repeated_speakers() {
        assert_eq!(
            expand_ocr_chat_line(
                "牡蛎: warrior牡蛎: BT / titan 牡蛎: overseer 牡蛎: commander 牡蛎: radical / agitator"
            ),
            vec![
                ParsedChatLine {
                    speaker: "牡蛎".to_owned(),
                    message: "warrior".to_owned(),
                },
                ParsedChatLine {
                    speaker: "牡蛎".to_owned(),
                    message: "BT / titan".to_owned(),
                },
                ParsedChatLine {
                    speaker: "牡蛎".to_owned(),
                    message: "overseer".to_owned(),
                },
                ParsedChatLine {
                    speaker: "牡蛎".to_owned(),
                    message: "commander".to_owned(),
                },
                ParsedChatLine {
                    speaker: "牡蛎".to_owned(),
                    message: "radical / agitator".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn splits_flattened_english_ocr_messages_without_speakers() {
        assert_eq!(
            expand_ocr_chat_line(
                "Super Earth is a pig ： Let 《 s go dO the mission. Help me out bro Nice! Let 《 S go ， evac now"
            ),
            vec![
                ParsedChatLine {
                    speaker: String::new(),
                    message: "Super Earth is a pig".to_owned(),
                },
                ParsedChatLine {
                    speaker: String::new(),
                    message: "Let's go do the mission.".to_owned(),
                },
                ParsedChatLine {
                    speaker: String::new(),
                    message: "Help me out bro".to_owned(),
                },
                ParsedChatLine {
                    speaker: String::new(),
                    message: "Nice!".to_owned(),
                },
                ParsedChatLine {
                    speaker: String::new(),
                    message: "Let's go, evac now".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn splits_flattened_ocr_with_case_and_space_noise() {
        assert_eq!(
            expand_ocr_chat_line(
                "Super Earth is a pig B ro rez me Letl s go dO the mission. Nice one bro charger"
            ),
            vec![
                ParsedChatLine {
                    speaker: String::new(),
                    message: "Super Earth is a pig".to_owned(),
                },
                ParsedChatLine {
                    speaker: String::new(),
                    message: "Bro rez me".to_owned(),
                },
                ParsedChatLine {
                    speaker: String::new(),
                    message: "Let's go do the mission.".to_owned(),
                },
                ParsedChatLine {
                    speaker: String::new(),
                    message: "Nice one bro".to_owned(),
                },
                ParsedChatLine {
                    speaker: String::new(),
                    message: "charger".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn ignores_speaker_only_ocr_crumbs() {
        assert!(expand_ocr_chat_line("牡蛎:").is_empty());
        assert_eq!(
            expand_ocr_chat_line("牡蛎: warrior 牡蛎:"),
            vec![ParsedChatLine {
                speaker: "牡蛎".to_owned(),
                message: "warrior".to_owned(),
            }]
        );
    }

    #[test]
    fn extracts_json_array_from_fenced_model_output() {
        assert_eq!(
            extract_json_array("```json\n[\"收到\",\"快走\"]\n```"),
            Some("[\"收到\",\"快走\"]")
        );
    }

    #[test]
    fn summarizes_non_utf8_response_bodies_without_panicking() {
        let expected = format!("OK{}", char::REPLACEMENT_CHARACTER);
        assert_eq!(response_body_summary(&[b'O', b'K', 0, 0xFF], 20), expected);
    }

    #[test]
    fn accepts_reasoning_only_responses_without_a_content_field() {
        let parsed: ChatResponse = serde_json::from_str(
            r#"{"choices":[{"finish_reason":"length","message":{"role":"assistant","reasoning_content":"thinking"}}]}"#,
        )
        .unwrap();
        let choice = parsed.choices.into_iter().next().unwrap();
        assert_eq!(choice.finish_reason.as_deref(), Some("length"));
        assert_eq!(choice.message.content, None);
        assert_eq!(
            choice.message.reasoning_content.as_deref(),
            Some("thinking")
        );
    }

    #[cfg(windows)]
    #[test]
    fn merges_configured_speaker_with_english_message_by_position() {
        use crate::platform::windows::capture::{OcrReading, PositionedOcrLine};
        let configured = OcrReading {
            language: "zh-Hans-CN".to_owned(),
            lines: vec![PositionedOcrLine {
                text: "牡蛎：超级地球简直就是个垃圾".to_owned(),
                top: 40.0,
                height: 20.0,
            }],
        };
        let english = OcrReading {
            language: "en-US".to_owned(),
            lines: vec![PositionedOcrLine {
                text: "牡蛎: Super Earth is just a pig.".to_owned(),
                top: 41.0,
                height: 19.0,
            }],
        };
        let primary = prefer_latin_ocr(&configured, &english);
        assert_eq!(primary.language, "en-US");
        assert_eq!(
            merge_ocr_chat_lines(&primary.lines, &configured.lines),
            vec![ParsedChatLine {
                speaker: "牡蛎".to_owned(),
                message: "Super Earth is just a pig.".to_owned(),
            }]
        );
    }

    #[cfg(windows)]
    #[test]
    fn merges_chinese_and_english_chat_lines_by_position() {
        use crate::platform::windows::capture::PositionedOcrLine;
        let chinese = vec![
            PositionedOcrLine {
                text: "牡蛎：快撤退".to_owned(),
                top: 10.0,
                height: 18.0,
            },
            PositionedOcrLine {
                text: "Eagle1: extraction ready".to_owned(),
                top: 40.0,
                height: 18.0,
            },
        ];
        let english = vec![PositionedOcrLine {
            text: "Eagle1: extraction ready!".to_owned(),
            top: 41.0,
            height: 18.0,
        }];

        assert_eq!(
            merge_bilingual_ocr_lines(&chinese, &english),
            vec![
                ParsedChatLine {
                    speaker: "牡蛎".to_owned(),
                    message: "快撤退".to_owned(),
                },
                ParsedChatLine {
                    speaker: "Eagle1".to_owned(),
                    message: "extraction ready!".to_owned(),
                },
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn propagates_one_detected_speaker_to_flattened_english_lines() {
        use crate::platform::windows::capture::PositionedOcrLine;
        let chinese = vec![PositionedOcrLine {
            text: "牡蛎: 超级地球是猪".to_owned(),
            top: 10.0,
            height: 18.0,
        }];
        let english = vec![PositionedOcrLine {
            text: "Super Earth is a pig: Let's go do the mission. Help me out bro Nice! Let's go, evac now"
                .to_owned(),
            top: 11.0,
            height: 18.0,
        }];

        let merged = merge_bilingual_ocr_lines(&chinese, &english);
        assert_eq!(merged.len(), 5);
        assert!(merged.iter().all(|line| line.speaker == "牡蛎"));
    }

    #[cfg(windows)]
    #[test]
    fn keeps_adjacent_unlabelled_chat_messages_separate() {
        use crate::platform::windows::capture::PositionedOcrLine;
        let chinese = vec![
            PositionedOcrLine {
                text: "牡蛎: Super Earth is a pig".to_owned(),
                top: 10.0,
                height: 18.0,
            },
            PositionedOcrLine {
                text: "b ro help".to_owned(),
                top: 42.0,
                height: 18.0,
            },
            PositionedOcrLine {
                text: "Let's go do the mission.".to_owned(),
                top: 74.0,
                height: 18.0,
            },
            PositionedOcrLine {
                text: "Nice, bro".to_owned(),
                top: 106.0,
                height: 18.0,
            },
            PositionedOcrLine {
                text: "charger".to_owned(),
                top: 138.0,
                height: 18.0,
            },
        ];

        assert_eq!(
            merge_bilingual_ocr_lines(&chinese, &[]),
            vec![
                ParsedChatLine {
                    speaker: "牡蛎".to_owned(),
                    message: "Super Earth is a pig".to_owned(),
                },
                ParsedChatLine {
                    speaker: "牡蛎".to_owned(),
                    message: "b ro help".to_owned(),
                },
                ParsedChatLine {
                    speaker: "牡蛎".to_owned(),
                    message: "Let's go do the mission.".to_owned(),
                },
                ParsedChatLine {
                    speaker: "牡蛎".to_owned(),
                    message: "Nice, bro".to_owned(),
                },
                ParsedChatLine {
                    speaker: "牡蛎".to_owned(),
                    message: "charger".to_owned(),
                },
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn merges_a_wrapped_ocr_line_into_the_previous_chat_message() {
        use crate::platform::windows::capture::PositionedOcrLine;
        let chinese = vec![
            PositionedOcrLine {
                text: "牡蛎: I marked the locations for the".to_owned(),
                top: 10.0,
                height: 18.0,
            },
            PositionedOcrLine {
                text: "tasks.".to_owned(),
                top: 29.0,
                height: 18.0,
            },
            PositionedOcrLine {
                text: "牡蛎: Why is an error being reported?".to_owned(),
                top: 52.0,
                height: 18.0,
            },
        ];

        assert_eq!(
            merge_bilingual_ocr_lines(&chinese, &[]),
            vec![
                ParsedChatLine {
                    speaker: "牡蛎".to_owned(),
                    message: "I marked the locations for the tasks.".to_owned(),
                },
                ParsedChatLine {
                    speaker: "牡蛎".to_owned(),
                    message: "Why is an error being reported?".to_owned(),
                },
            ]
        );
    }

    fn parsed(message: &str) -> ParsedChatLine {
        ParsedChatLine {
            speaker: "Player".to_owned(),
            message: message.to_owned(),
        }
    }

    #[test]
    fn tracks_only_appended_chat_lines() {
        let mut tracker = ChatLineTracker::default();
        let first = vec![parsed("one"), parsed("two")];
        assert_eq!(tracker.pending_lines(7, &first), first);
        tracker.commit(7, &first);

        let next = vec![parsed("one"), parsed("two"), parsed("three")];
        assert_eq!(tracker.pending_lines(7, &next), vec![parsed("three")]);
    }

    #[test]
    fn tracks_new_lines_after_the_chat_window_scrolls() {
        let mut tracker = ChatLineTracker::default();
        let previous = vec![parsed("one"), parsed("two"), parsed("three")];
        tracker.commit(3, &previous);

        let current = vec![
            parsed("two"),
            parsed("three"),
            parsed("four"),
            parsed("five"),
        ];
        assert_eq!(
            tracker.pending_lines(3, &current),
            vec![parsed("four"), parsed("five")]
        );
    }

    #[test]
    fn treats_an_appended_duplicate_message_as_new() {
        let mut tracker = ChatLineTracker::default();
        tracker.commit(3, &[parsed("same message")]);

        assert_eq!(
            tracker.pending_lines(3, &[parsed("same message"), parsed("same message")]),
            vec![parsed("same message")]
        );
    }

    #[test]
    fn ignores_minor_ocr_spacing_and_punctuation_changes() {
        let mut tracker = ChatLineTracker::default();
        tracker.commit(2, &[parsed("Super Earth, forever!")]);

        assert!(
            tracker
                .pending_lines(2, &[parsed("Super  Earth forever")])
                .is_empty()
        );
    }

    #[test]
    fn starts_fresh_for_a_new_target_generation() {
        let mut tracker = ChatLineTracker::default();
        tracker.commit(1, &[parsed("old")]);

        assert_eq!(
            tracker.pending_lines(2, &[parsed("old")]),
            vec![parsed("old")]
        );
    }
}
