export type StratagemPresetGroup = 'support' | 'orbital' | 'eagle' | 'emplacement' | 'sentry' | 'backpack' | 'vehicle' | 'mission'

export interface StratagemPreset {
  id: string
  group: StratagemPresetGroup
  zhName: string
  enName: string
  sequence: string[]
}

export const STRATAGEM_GROUP_LABELS: Record<StratagemPresetGroup | 'all', string> = {
  "all": "全部",
  "support": "支援武器",
  "orbital": "轨道系",
  "eagle": "飞鹰系",
  "emplacement": "防御系",
  "sentry": "哨戒炮",
  "backpack": "背包",
  "vehicle": "载具",
  "mission": "任务通用"
}

export const STRATAGEM_PRESETS: StratagemPreset[] = [
  {
    "id": "wpn_mg",
    "group": "support",
    "zhName": "MG-43 机枪",
    "enName": "MG-43 Machine Gun",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyS",
      "KeyW",
      "KeyD"
    ]
  },
  {
    "id": "wpn_eat",
    "group": "support",
    "zhName": "EAT-17 消耗性反坦克武器",
    "enName": "EAT-17 Expendable Anti-Tank",
    "sequence": [
      "KeyS",
      "KeyS",
      "KeyA",
      "KeyW",
      "KeyD"
    ]
  },
  {
    "id": "wpn_stalwart",
    "group": "support",
    "zhName": "M-105 盟友",
    "enName": "M-105 Stalwart",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyS",
      "KeyW",
      "KeyW",
      "KeyA"
    ]
  },
  {
    "id": "wpn_laser",
    "group": "support",
    "zhName": "LAS-98 激光大炮",
    "enName": "LAS-98 Laser Cannon",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyS",
      "KeyW",
      "KeyA"
    ]
  },
  {
    "id": "wpn_amr",
    "group": "support",
    "zhName": "APW-1 反器材步枪",
    "enName": "APW-1 Anti-Materiel Rifle",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyD",
      "KeyW",
      "KeyS"
    ]
  },
  {
    "id": "wpn_rr",
    "group": "support",
    "zhName": "GR-8 无后坐力炮",
    "enName": "GR-8 Recoilless Rifle",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyD",
      "KeyD",
      "KeyA"
    ]
  },
  {
    "id": "wpn_gl",
    "group": "support",
    "zhName": "GL-21 榴弹发射器",
    "enName": "GL-21 Grenade Launcher",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyW",
      "KeyA",
      "KeyS"
    ]
  },
  {
    "id": "wpn_flame",
    "group": "support",
    "zhName": "FLAM-40 火焰喷射器",
    "enName": "FLAM-40 Flamethrower",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyW",
      "KeyS",
      "KeyW"
    ]
  },
  {
    "id": "wpn_hmg",
    "group": "support",
    "zhName": "MG-206 重机枪",
    "enName": "MG-206 Heavy Machine Gun",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyW",
      "KeyS",
      "KeyS"
    ]
  },
  {
    "id": "wpn_ac",
    "group": "support",
    "zhName": "AC-8 机炮",
    "enName": "AC-8 Autocannon",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyS",
      "KeyW",
      "KeyW",
      "KeyD"
    ]
  },
  {
    "id": "wpn_arc",
    "group": "support",
    "zhName": "ARC-3 电弧发射器",
    "enName": "ARC-3 Arc Thrower",
    "sequence": [
      "KeyS",
      "KeyD",
      "KeyS",
      "KeyW",
      "KeyA",
      "KeyA"
    ]
  },
  {
    "id": "wpn_quasar",
    "group": "support",
    "zhName": "LAS-99类星体加农炮",
    "enName": "LAS-99 Quasar Cannon",
    "sequence": [
      "KeyS",
      "KeyS",
      "KeyW",
      "KeyA",
      "KeyD"
    ]
  },
  {
    "id": "wpn_airburst",
    "group": "support",
    "zhName": "RL-77 空爆火箭弹发射器",
    "enName": "RL-77 Airburst Rocket Launcher",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyW",
      "KeyA",
      "KeyD"
    ]
  },
  {
    "id": "wpn_commando",
    "group": "support",
    "zhName": "MLS-4X 突击兵",
    "enName": "MLS-4X Commando",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyW",
      "KeyS",
      "KeyD"
    ]
  },
  {
    "id": "wpn_spear",
    "group": "support",
    "zhName": "FAF-14 飞矛",
    "enName": "FAF-14 Spear",
    "sequence": [
      "KeyS",
      "KeyS",
      "KeyW",
      "KeyS",
      "KeyS"
    ]
  },
  {
    "id": "wpn_railgun",
    "group": "support",
    "zhName": "RS-422 磁轨炮",
    "enName": "RS-422 Railgun",
    "sequence": [
      "KeyS",
      "KeyD",
      "KeyS",
      "KeyW",
      "KeyA",
      "KeyD"
    ]
  },
  {
    "id": "wpn_wasp",
    "group": "support",
    "zhName": "StA-X3 W.A.S.P. 发射器",
    "enName": "StA-X3 W.A.S.P. Launcher",
    "sequence": [
      "KeyS",
      "KeyS",
      "KeyW",
      "KeyS",
      "KeyD"
    ]
  },
  {
    "id": "wpn_breach",
    "group": "support",
    "zhName": "CQC-20 破门锤",
    "enName": "CQC-20 Breaching Hammer",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyD",
      "KeyA",
      "KeyW"
    ]
  },
  {
    "id": "wpn_epoch",
    "group": "support",
    "zhName": "PLAS-45 纪元",
    "enName": "PLAS-45 Epoch",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyW",
      "KeyA",
      "KeyD"
    ]
  },
  {
    "id": "wpn_speargun",
    "group": "support",
    "zhName": "S-11 矛枪",
    "enName": "S-11 Speargun",
    "sequence": [
      "KeyS",
      "KeyD",
      "KeyS",
      "KeyA",
      "KeyW",
      "KeyD"
    ]
  },
  {
    "id": "wpn_defoliator",
    "group": "support",
    "zhName": "CQC-9 除叶工具",
    "enName": "CQC-9 Defoliation Tool",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyD",
      "KeyD",
      "KeyS"
    ]
  },
  {
    "id": "wpn_leveller",
    "group": "support",
    "zhName": "EAT-411 荡平者",
    "enName": "EAT-411 Leveller",
    "sequence": [
      "KeyS",
      "KeyS",
      "KeyA",
      "KeyW",
      "KeyS"
    ]
  },
  {
    "id": "wpn_sterilizer",
    "group": "support",
    "zhName": "TX-41 灭菌器",
    "enName": "TX-41 Sterilizer",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyW",
      "KeyS",
      "KeyA"
    ]
  },
  {
    "id": "wpn_deescalator",
    "group": "support",
    "zhName": "GL-52 缓和使者",
    "enName": "GL-52 De-Escalator",
    "sequence": [
      "KeyS",
      "KeyD",
      "KeyW",
      "KeyA",
      "KeyD"
    ]
  },
  {
    "id": "wpn_napalm",
    "group": "support",
    "zhName": "EAT-700 消耗性凝固汽油弹",
    "enName": "EAT-700 Expendable Napalm",
    "sequence": [
      "KeyS",
      "KeyS",
      "KeyA",
      "KeyW",
      "KeyA"
    ]
  },
  {
    "id": "wpn_beltgl",
    "group": "support",
    "zhName": "GL-28 弹链式榴弹发射器",
    "enName": "GL-28 Belt-Fed Grenade Launcher",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyW",
      "KeyA",
      "KeyW",
      "KeyW"
    ]
  },
  {
    "id": "wpn_maxigun",
    "group": "support",
    "zhName": "M-1000 重装机枪",
    "enName": "M-1000 Maxigun",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyD",
      "KeyS",
      "KeyW",
      "KeyW"
    ]
  },
  {
    "id": "wpn_silo",
    "group": "support",
    "zhName": "MS-11 单兵导弹发射井",
    "enName": "MS-11 Solo Silo",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyD",
      "KeyS",
      "KeyS"
    ]
  },
  {
    "id": "wpn_flag",
    "group": "support",
    "zhName": "CQC-1 唯一真旗",
    "enName": "CQC-1 One True Flag",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyD",
      "KeyD",
      "KeyW"
    ]
  },
  {
    "id": "wpn_flam80",
    "group": "support",
    "zhName": "B/FLAM-80 焚燃者",
    "enName": "B/FLAM-80 Cremator",
    "sequence": [
      "KeyS",
      "KeyS",
      "KeyD",
      "KeyS",
      "KeyW",
      "KeyW"
    ]
  },
  {
    "id": "wpn_c4",
    "group": "support",
    "zhName": "B/MD C4背包",
    "enName": "B/MD C4 Pack",
    "sequence": [
      "KeyS",
      "KeyD",
      "KeyW",
      "KeyW",
      "KeyD",
      "KeyW"
    ]
  },
  {
    "id": "wpn_bulletstorm",
    "group": "support",
    "zhName": "MGX-42 子弹风暴",
    "enName": "MGX-42 Bullet Storm",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyS",
      "KeyD",
      "KeyW",
      "KeyA"
    ]
  },
  {
    "id": "wpn_meltagun",
    "group": "support",
    "zhName": "40-K热熔枪",
    "enName": "40-KMeltagun",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyW",
      "KeyA",
      "KeyA",
      "KeyS"
    ]
  },
  {
    "id": "orb_precision",
    "group": "orbital",
    "zhName": "轨道精准攻击",
    "enName": "Orbital Precision Strike",
    "sequence": [
      "KeyD",
      "KeyD",
      "KeyW"
    ]
  },
  {
    "id": "orb_gatling",
    "group": "orbital",
    "zhName": "轨道加特林火力网",
    "enName": "Orbital Gatling Barrage",
    "sequence": [
      "KeyD",
      "KeyS",
      "KeyA",
      "KeyW",
      "KeyW"
    ]
  },
  {
    "id": "orb_gas",
    "group": "orbital",
    "zhName": "轨道毒气攻击",
    "enName": "Orbital Gas Strike",
    "sequence": [
      "KeyD",
      "KeyD",
      "KeyS",
      "KeyD"
    ]
  },
  {
    "id": "orb_120",
    "group": "orbital",
    "zhName": "轨道120mm高爆弹火力网",
    "enName": "Orbital 120mm HE Barrage",
    "sequence": [
      "KeyD",
      "KeyD",
      "KeyS",
      "KeyA",
      "KeyD",
      "KeyS"
    ]
  },
  {
    "id": "orb_airburst",
    "group": "orbital",
    "zhName": "轨道空爆攻击",
    "enName": "Orbital Airburst Strike",
    "sequence": [
      "KeyD",
      "KeyD",
      "KeyD"
    ]
  },
  {
    "id": "orb_smoke",
    "group": "orbital",
    "zhName": "轨道烟雾攻击",
    "enName": "Orbital Smoke Strike",
    "sequence": [
      "KeyD",
      "KeyD",
      "KeyS",
      "KeyW"
    ]
  },
  {
    "id": "orb_ems",
    "group": "orbital",
    "zhName": "轨道电磁冲击波攻击",
    "enName": "Orbital EMS Strike",
    "sequence": [
      "KeyD",
      "KeyD",
      "KeyA",
      "KeyS"
    ]
  },
  {
    "id": "orb_380",
    "group": "orbital",
    "zhName": "轨道380mm高爆弹火力网",
    "enName": "Orbital 380mm HE Barrage",
    "sequence": [
      "KeyD",
      "KeyS",
      "KeyW",
      "KeyW",
      "KeyA",
      "KeyS",
      "KeyS"
    ]
  },
  {
    "id": "orb_walking",
    "group": "orbital",
    "zhName": "轨道游走火力网",
    "enName": "Orbital Walking Barrage",
    "sequence": [
      "KeyD",
      "KeyS",
      "KeyD",
      "KeyS",
      "KeyD",
      "KeyS"
    ]
  },
  {
    "id": "orb_laser",
    "group": "orbital",
    "zhName": "轨道激光炮",
    "enName": "Orbital Laser",
    "sequence": [
      "KeyD",
      "KeyS",
      "KeyW",
      "KeyD",
      "KeyS"
    ]
  },
  {
    "id": "orb_napalm",
    "group": "orbital",
    "zhName": "轨道凝固汽油弹火力网",
    "enName": "Orbital Napalm Barrage",
    "sequence": [
      "KeyD",
      "KeyD",
      "KeyS",
      "KeyA",
      "KeyD",
      "KeyW"
    ]
  },
  {
    "id": "orb_railcannon",
    "group": "orbital",
    "zhName": "轨道炮攻击",
    "enName": "Orbital Railcannon Strike",
    "sequence": [
      "KeyD",
      "KeyW",
      "KeyS",
      "KeyS",
      "KeyD"
    ]
  },
  {
    "id": "eagle_strafing",
    "group": "eagle",
    "zhName": "“飞鹰”机枪扫射",
    "enName": "Eagle Strafing Run",
    "sequence": [
      "KeyW",
      "KeyD",
      "KeyD"
    ]
  },
  {
    "id": "eagle_airstrike",
    "group": "eagle",
    "zhName": "“飞鹰”空袭",
    "enName": "Eagle Airstrike",
    "sequence": [
      "KeyW",
      "KeyD",
      "KeyS",
      "KeyD"
    ]
  },
  {
    "id": "eagle_cluster",
    "group": "eagle",
    "zhName": "“飞鹰”集束炸弹",
    "enName": "Eagle Cluster Bomb",
    "sequence": [
      "KeyW",
      "KeyD",
      "KeyS",
      "KeyS",
      "KeyD"
    ]
  },
  {
    "id": "eagle_smoke",
    "group": "eagle",
    "zhName": "“飞鹰”烟雾攻击",
    "enName": "Eagle Smoke Strike",
    "sequence": [
      "KeyW",
      "KeyD",
      "KeyW",
      "KeyS"
    ]
  },
  {
    "id": "eagle_napalm",
    "group": "eagle",
    "zhName": "“飞鹰”凝固汽油弹空袭",
    "enName": "Eagle Napalm Airstrike",
    "sequence": [
      "KeyW",
      "KeyD",
      "KeyS",
      "KeyW"
    ]
  },
  {
    "id": "eagle_110",
    "group": "eagle",
    "zhName": "“飞鹰”110MM火箭巢",
    "enName": "Eagle 110mm Rocket Pods",
    "sequence": [
      "KeyW",
      "KeyD",
      "KeyW",
      "KeyA"
    ]
  },
  {
    "id": "eagle_500kg",
    "group": "eagle",
    "zhName": "“飞鹰”500KG炸弹",
    "enName": "Eagle 500kg Bomb",
    "sequence": [
      "KeyW",
      "KeyD",
      "KeyS",
      "KeyS",
      "KeyS"
    ]
  },
  {
    "id": "emp_mine_ap",
    "group": "emplacement",
    "zhName": "MD-6 反步兵雷区",
    "enName": "MD-6 Anti-Personnel Minefield",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyW",
      "KeyD"
    ]
  },
  {
    "id": "emp_mine_inc",
    "group": "emplacement",
    "zhName": "MD-I4 燃烧地雷",
    "enName": "MD-I4 Incendiary Mines",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyA",
      "KeyS"
    ]
  },
  {
    "id": "emp_mine_at",
    "group": "emplacement",
    "zhName": "MD-17 反坦克地雷",
    "enName": "MD-17 Anti-Tank Mines",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyW",
      "KeyW"
    ]
  },
  {
    "id": "emp_shield",
    "group": "emplacement",
    "zhName": "FX-12 防护罩生成中继器",
    "enName": "FX-12 Shield Generator Relay",
    "sequence": [
      "KeyS",
      "KeyS",
      "KeyA",
      "KeyD",
      "KeyA",
      "KeyD"
    ]
  },
  {
    "id": "emp_hmg",
    "group": "emplacement",
    "zhName": "E/MG-101 重机枪部署支架",
    "enName": "E/MG-101 HMG Emplacement",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyA",
      "KeyD",
      "KeyD",
      "KeyA"
    ]
  },
  {
    "id": "emp_grenadier",
    "group": "emplacement",
    "zhName": "E/GL-21 掷弹兵防卫墙",
    "enName": "E/GL-21 Grenadier Battlement",
    "sequence": [
      "KeyS",
      "KeyD",
      "KeyS",
      "KeyA",
      "KeyD"
    ]
  },
  {
    "id": "emp_mine_gas",
    "group": "emplacement",
    "zhName": "MD-8 毒气地雷",
    "enName": "MD-8 Gas Mines",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyA",
      "KeyD"
    ]
  },
  {
    "id": "emp_at",
    "group": "emplacement",
    "zhName": "E/AT-12 反坦克炮台",
    "enName": "E/AT-12 Anti-Tank Emplacement",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyA",
      "KeyD",
      "KeyD",
      "KeyD"
    ]
  },
  {
    "id": "sen_mg",
    "group": "sentry",
    "zhName": "A/MG-43 哨戒机枪",
    "enName": "A/MG-43 Machine Gun Sentry",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyD",
      "KeyD",
      "KeyW"
    ]
  },
  {
    "id": "sen_gatling",
    "group": "sentry",
    "zhName": "A/G-16 加特林哨戒炮",
    "enName": "A/G-16 Gatling Sentry",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyD",
      "KeyA"
    ]
  },
  {
    "id": "sen_ac",
    "group": "sentry",
    "zhName": "A/AC-8 自动哨戒炮",
    "enName": "A/AC-8 Autocannon Sentry",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyD",
      "KeyW",
      "KeyA",
      "KeyW"
    ]
  },
  {
    "id": "sen_mortar",
    "group": "sentry",
    "zhName": "A/M-12 迫击哨戒炮",
    "enName": "A/M-12 Mortar Sentry",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyD",
      "KeyD",
      "KeyS"
    ]
  },
  {
    "id": "sen_rocket",
    "group": "sentry",
    "zhName": "A/MLS-4X 火箭哨戒炮",
    "enName": "A/MLS-4X Rocket Sentry",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyD",
      "KeyD",
      "KeyA"
    ]
  },
  {
    "id": "sen_tesla",
    "group": "sentry",
    "zhName": "A/ARC-3 特斯拉塔",
    "enName": "A/ARC-3 Tesla Tower",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyD",
      "KeyW",
      "KeyA",
      "KeyD"
    ]
  },
  {
    "id": "sen_ems",
    "group": "sentry",
    "zhName": "A/M-23 电磁冲击波迫击哨戒炮",
    "enName": "A/M-23 EMS Mortar Sentry",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyD",
      "KeyS",
      "KeyD"
    ]
  },
  {
    "id": "sen_flame",
    "group": "sentry",
    "zhName": "A/FLAM-40 火焰喷射哨戒炮",
    "enName": "A/FLAM-40 Flame Sentry",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyD",
      "KeyS",
      "KeyW",
      "KeyW"
    ]
  },
  {
    "id": "sen_laser",
    "group": "sentry",
    "zhName": "A/LAS-98 激光哨戒炮",
    "enName": "A/LAS-98 Laser Sentry",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyD",
      "KeyS",
      "KeyW",
      "KeyD"
    ]
  },
  {
    "id": "sen_gas",
    "group": "sentry",
    "zhName": "A/GM-17 瓦斯迫击哨戒炮",
    "enName": "A/GM-17 Gas Mortar Sentry",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyD",
      "KeyS",
      "KeyA"
    ]
  },
  {
    "id": "pack_supply",
    "group": "backpack",
    "zhName": "B-1 补给背包",
    "enName": "B-1 Supply Pack",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyS",
      "KeyW",
      "KeyW",
      "KeyS"
    ]
  },
  {
    "id": "pack_jump",
    "group": "backpack",
    "zhName": "LIFT-850 喷射背包",
    "enName": "LIFT-850 Jump Pack",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyW",
      "KeyS",
      "KeyW"
    ]
  },
  {
    "id": "pack_ballistic",
    "group": "backpack",
    "zhName": "SH-20 防弹护盾背包",
    "enName": "SH-20 Ballistic Shield Backpack",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyS",
      "KeyS",
      "KeyW",
      "KeyA"
    ]
  },
  {
    "id": "pack_guard",
    "group": "backpack",
    "zhName": "AX/AR-23 “护卫犬”",
    "enName": "AX/AR-23 Guard Dog",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyA",
      "KeyW",
      "KeyD",
      "KeyS"
    ]
  },
  {
    "id": "pack_rover",
    "group": "backpack",
    "zhName": "AX/LAS-5 漫游车",
    "enName": "AX/LAS-5 Rover",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyA",
      "KeyW",
      "KeyD",
      "KeyD"
    ]
  },
  {
    "id": "pack_shield",
    "group": "backpack",
    "zhName": "SH-32 防护罩生成包",
    "enName": "SH-32 Shield Generator Pack",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyA",
      "KeyD",
      "KeyA",
      "KeyD"
    ]
  },
  {
    "id": "pack_dir_shield",
    "group": "backpack",
    "zhName": "SH-51 定向护盾",
    "enName": "SH-51 Directional Shield",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyA",
      "KeyD",
      "KeyW",
      "KeyW"
    ]
  },
  {
    "id": "pack_hotdog",
    "group": "backpack",
    "zhName": "AX/FLAM-75 热狗",
    "enName": "AX/FLAM-75 Hot Dog",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyA",
      "KeyW",
      "KeyA",
      "KeyA"
    ]
  },
  {
    "id": "pack_portable_hb",
    "group": "backpack",
    "zhName": "B-100 便携式地狱火炸弹",
    "enName": "B-100 Portable Hellbomb",
    "sequence": [
      "KeyS",
      "KeyD",
      "KeyW",
      "KeyW",
      "KeyW"
    ]
  },
  {
    "id": "pack_k9",
    "group": "backpack",
    "zhName": "AX/ARC-3 K-9",
    "enName": "AX/ARC-3 K-9",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyA",
      "KeyW",
      "KeyD",
      "KeyA"
    ]
  },
  {
    "id": "pack_hover",
    "group": "backpack",
    "zhName": "LIFT-860 悬浮背包",
    "enName": "LIFT-860 Hover Pack",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyW",
      "KeyS",
      "KeyA",
      "KeyD"
    ]
  },
  {
    "id": "pack_dog_breath",
    "group": "backpack",
    "zhName": "AX/TX-13 腐息",
    "enName": "AX/TX-13 Dog Breath",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyA",
      "KeyW",
      "KeyD",
      "KeyW"
    ]
  },
  {
    "id": "pack_warp",
    "group": "backpack",
    "zhName": "LIFT-182 传送背包",
    "enName": "LIFT-182 Warp Pack",
    "sequence": [
      "KeyS",
      "KeyA",
      "KeyD",
      "KeyS",
      "KeyA",
      "KeyD"
    ]
  },
  {
    "id": "veh_patriot",
    "group": "vehicle",
    "zhName": "EXO-45 “爱国者” 外骨骼装甲",
    "enName": "EXO-45 Patriot Exosuit",
    "sequence": [
      "KeyA",
      "KeyS",
      "KeyD",
      "KeyW",
      "KeyA",
      "KeyS",
      "KeyS"
    ]
  },
  {
    "id": "veh_emancipator",
    "group": "vehicle",
    "zhName": "EXO-49 “解放者” 外骨骼装甲",
    "enName": "EXO-49 Emancipator Exosuit",
    "sequence": [
      "KeyA",
      "KeyS",
      "KeyD",
      "KeyW",
      "KeyA",
      "KeyS",
      "KeyW"
    ]
  },
  {
    "id": "veh_Breakthrough",
    "group": "vehicle",
    "zhName": "EXO-55 “突破” 外骨骼装甲",
    "enName": "EXO-55 Breakthrough Exosuit",
    "sequence": [
      "KeyA",
      "KeyS",
      "KeyD",
      "KeyA",
      "KeyD",
      "KeyS",
      "KeyW"
    ]
  },
  {
    "id": "veh_Lumberer",
    "group": "vehicle",
    "zhName": "EXO-51 “伐木者” 外骨骼装甲",
    "enName": "EXO-51 Lumberer Exosuit",
    "sequence": [
      "KeyA",
      "KeyS",
      "KeyD",
      "KeyW",
      "KeyD",
      "KeyA",
      "KeyW"
    ]
  },
  {
    "id": "veh_recon",
    "group": "vehicle",
    "zhName": "M-102 快速侦察载具",
    "enName": "M-102 Fast Recon Vehicle",
    "sequence": [
      "KeyA",
      "KeyS",
      "KeyD",
      "KeyS",
      "KeyD",
      "KeyS",
      "KeyW"
    ]
  },
  {
    "id": "veh_supply",
    "group": "vehicle",
    "zhName": "M-103 补给型快速侦察载具",
    "enName": "M-103 Supply FRV",
    "sequence": [
      "KeyA",
      "KeyS",
      "KeyA",
      "KeyA",
      "KeyS",
      "KeyW",
      "KeyD"
    ]
  },
  {
    "id": "veh_incinerator",
    "group": "vehicle",
    "zhName": "M-104 炽灼型快速侦察载具",
    "enName": "M-104 Incinerator FRV",
    "sequence": [
      "KeyA",
      "KeyS",
      "KeyD",
      "KeyA",
      "KeyS",
      "KeyW",
      "KeyW"
    ]
  },
  {
    "id": "veh_bastion",
    "group": "vehicle",
    "zhName": "TD-220 堡垒 MK XVI",
    "enName": "TD-220 Bastion MK XVI",
    "sequence": [
      "KeyA",
      "KeyS",
      "KeyD",
      "KeyS",
      "KeyA",
      "KeyS",
      "KeyW",
      "KeyS",
      "KeyW"
    ]
  },
  {
    "id": "mis_sos",
    "group": "mission",
    "zhName": "SOS救援信标",
    "enName": "SOS Beacon",
    "sequence": [
      "KeyW",
      "KeyS",
      "KeyD",
      "KeyW"
    ]
  },
  {
    "id": "mis_resupply",
    "group": "mission",
    "zhName": "重新补给",
    "enName": "Resupply",
    "sequence": [
      "KeyS",
      "KeyS",
      "KeyW",
      "KeyD"
    ]
  },
  {
    "id": "mis_reinforce",
    "group": "mission",
    "zhName": "增援",
    "enName": "Reinforce",
    "sequence": [
      "KeyW",
      "KeyS",
      "KeyD",
      "KeyA",
      "KeyW"
    ]
  },
  {
    "id": "mis_hellbomb",
    "group": "mission",
    "zhName": "呼叫地狱火炸弹",
    "enName": "Hellbomb",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyA",
      "KeyS",
      "KeyW",
      "KeyD",
      "KeyS",
      "KeyW"
    ]
  },
  {
    "id": "mis_flag",
    "group": "mission",
    "zhName": "呼叫超级地球旗帜",
    "enName": "Super Earth Flag",
    "sequence": [
      "KeyS",
      "KeyW",
      "KeyS",
      "KeyW"
    ]
  },
  {
    "id": "mis_eagleresupply",
    "group": "mission",
    "zhName": "重新武装 “飞鹰”",
    "enName": "Eagle Rearm",
    "sequence": [
      "KeyW",
      "KeyW",
      "KeyA",
      "KeyW",
      "KeyD"
    ]
  },
  {
    "id": "mis_seafartillery",
    "group": "mission",
    "zhName": "超级地球武装部队大炮",
    "enName": "SEAF Artillery",
    "sequence": [
      "KeyD",
      "KeyW",
      "KeyW",
      "KeyS"
    ]
  },
  {
    "id": "mis_callinsys",
    "group": "mission",
    "zhName": "呼叫超级驱逐舰",
    "enName": "Call In Super Destroyer",
    "sequence": [
      "KeyW",
      "KeyW",
      "KeyS",
      "KeyS",
      "KeyA",
      "KeyD",
      "KeyA",
      "KeyD"
    ]
  },
  {
    "id": "mis_cargo",
    "group": "mission",
    "zhName": "呼叫货柜",
    "enName": "Cargo Container",
    "sequence": [
      "KeyW",
      "KeyW",
      "KeyS",
      "KeyS",
      "KeyD",
      "KeyS"
    ]
  },
  {
    "id": "mis_updd",
    "group": "mission",
    "zhName": "上传逃生舱数据",
    "enName": "Upload Data",
    "sequence": [
      "KeyA",
      "KeyD",
      "KeyW",
      "KeyW",
      "KeyW"
    ]
  }
]
