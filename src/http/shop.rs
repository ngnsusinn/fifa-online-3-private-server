use crate::state;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};

// Bunch options: 1 or 10 (5% discount)
pub const PACK_BUNCHES: &[u64] = &[1, 10];

// Recommendations: "new" order & "best" order (1-indexed rank)
pub const SHOP_NEW: &[u64] = &[45011, 45004, 50121, 50881, 501];
pub const SHOP_BEST: &[u64] = &[82214180, 8223600, 28001, 28003, 8223601];

// Mileage / MP Shop (shopid = MP_SHOPID + itemid)
pub const MP_SHOP: &[u64] = &[501, 50123, 50122, 45013, 45006, 3002];
pub const MP_SHOPID: u64 = 900_000_000;

// Special Bonus (shopid = STSHOPID + itemid, stid = STSHOPID + itemid)
pub const SPECIAL: &[u64] = &[8335, 8194, 50841, 50881];
pub const STSHOPID: u64 = 910_000_000;

// GP Shop (shopid = GPSHOPID + itemid)
pub const GPSHOP_ITEMS: &[u64] = &[50121, 50122, 50123, 50124, 50125];
pub const GPSHOPID: u64 = 920_000_000;

// Chained packages (first step -> all steps in order)
pub const CHAINED: &[(u64, &[u64])] = &[
    (70000001, &[70000001, 70000002, 70000003]),
];

#[derive(Debug, Clone)]
pub struct UseItemDef {
    pub itemid: u64,
    pub name: &'static str,
    pub icon: &'static str,
    pub item_type: &'static str,
    pub price: u64,
    pub description: &'static str,
}

pub const USE_ITEMS: &[UseItemDef] = &[
    UseItemDef { itemid: 501, name: "Lucky EP", icon: "501", item_type: "EP_LOTTERY", price: 50_000, description: "Buka kartu: dapat 10.000 - 1.000.000 EP." },
    UseItemDef { itemid: 50029, name: "Lucky EP max 20.000.000", icon: "ID50029", item_type: "EP_LOTTERY", price: 1_000_000, description: "Buka kartu: dapat 100.000 - 20.000.000 EP." },
    UseItemDef { itemid: 50028, name: "Lucky EP max 50.000.000", icon: "ID50028", item_type: "EP_LOTTERY", price: 2_500_000, description: "Buka kartu: dapat 200.000 - 50.000.000 EP." },
    UseItemDef { itemid: 908, name: "Jutawan EP Lottery", icon: "908", item_type: "EP_LOTTERY", price: 3_000_000, description: "Undian EP: 1 - 10 juta EP." },
    UseItemDef { itemid: 50019, name: "EP +200.000", icon: "ID50019", item_type: "EP_LOTTERY", price: 200_000, description: "Langsung dapat 200.000 EP." },
    UseItemDef { itemid: 50020, name: "EP +500.000", icon: "ID50020", item_type: "EP_LOTTERY", price: 500_000, description: "Langsung dapat 500.000 EP." },
    UseItemDef { itemid: 50021, name: "EP +1.000.000", icon: "ID50021", item_type: "EP_LOTTERY", price: 1_000_000, description: "Langsung dapat 1.000.000 EP." },
    UseItemDef { itemid: 50840, name: "Kantong EP", icon: "ID50840", item_type: "EP_LOTTERY", price: 1_500_000, description: "Isi 500.000 - 5.000.000 EP." },
    UseItemDef { itemid: 50841, name: "Peti EP", icon: "ID50841", item_type: "EP_LOTTERY", price: 10_000_000, description: "Isi 5 - 30 juta EP." },
    UseItemDef { itemid: 119082304, name: "Mesin Jackpot EP", icon: "119082304", item_type: "EP_LOTTERY", price: 5_000_000, description: "Jackpot: 100.000 - 100.000.000 EP." },
    UseItemDef { itemid: 50121, name: "Kotak Platinum", icon: "ID50121GP", item_type: "ITEM_LOTTERY", price: 10_000_000, description: "Hadiah acak: World Best 5, World Legend Top 30, Top 300, This Season Top 10 atau 10 juta EP." },
    UseItemDef { itemid: 50122, name: "Kotak Emas", icon: "ID50122GP", item_type: "ITEM_LOTTERY", price: 5_000_000, description: "Hadiah acak: World Best 50 2017, This Season Top 20 atau 3 juta EP." },
    UseItemDef { itemid: 50123, name: "Kotak Perak", icon: "ID50123GP", item_type: "ITEM_LOTTERY", price: 2_000_000, description: "Hadiah acak: pak FW/MF/DF 65+ atau 1 juta EP." },
    UseItemDef { itemid: 50124, name: "Kotak Perunggu", icon: "ID50124GP", item_type: "ITEM_LOTTERY", price: 500_000, description: "Hadiah acak: pak FW/MF/DF 60+ atau 300.000 EP." },
    UseItemDef { itemid: 50125, name: "Kotak Metal", icon: "ID50125GP", item_type: "ITEM_LOTTERY", price: 200_000, description: "Hadiah acak: pak 07 Top 200 atau 100.000 EP." },
    UseItemDef { itemid: 200130000, name: "Kotak Hadiah Perak", icon: "200130000", item_type: "ITEM_LOTTERY", price: 1_000_000, description: "Hadiah acak: 200.000 EP, pak GK 60+ atau World Best 300 2017." },
    UseItemDef { itemid: 200130001, name: "Kotak Hadiah Emas", icon: "200130001", item_type: "ITEM_LOTTERY", price: 3_000_000, description: "Hadiah acak: 1 juta EP, 06-11 Player Top 50 atau This Season Top 30." },
    UseItemDef { itemid: 3002, name: "Ganti nama manager", icon: "3002", item_type: "COACHNAME_CHANGE", price: 500_000, description: "Ganti nama manager Anda." },
    UseItemDef { itemid: 3001, name: "Ganti tim", icon: "3001", item_type: "TEAM_CHANGE", price: 100_000, description: "Pindah ke klub lain (logo, seragam, liga); pemain tetap." },
    UseItemDef { itemid: 1000052, name: "Enhancement Boost Gold", icon: "1000002", item_type: "ENHANCEMENT_BOOST", price: 1_500_000, description: "Upgrade: peluang sukses +15%." },
    UseItemDef { itemid: 1000053, name: "Enhancement Boost Silver", icon: "1000002", item_type: "ENHANCEMENT_BOOST", price: 1_000_000, description: "Upgrade: peluang sukses +10%." },
    UseItemDef { itemid: 1000054, name: "Enhancement Boost Bronze", icon: "1000002", item_type: "ENHANCEMENT_BOOST", price: 500_000, description: "Upgrade: peluang sukses +5%." },
    UseItemDef { itemid: 2001, name: "Simulasi Liga", icon: "2001", item_type: "LEAGUE_SIMULATION", price: 50_000, description: "Liga: hasil satu pertandingan langsung, tanpa main." },
];

#[derive(Debug, Clone)]
pub struct PackageDef {
    pub itemid: u64,
    pub name: &'static str,
    pub icon: &'static str,
    pub price: u64,
    pub contents: &'static [(u64, u64)],
    pub description: &'static str,
}

pub const PACKAGES: &[PackageDef] = &[
    PackageDef { itemid: 801, name: "Paket Starter", icon: "801", price: 1_000_000, contents: &[(28006, 1), (28007, 1), (28008, 1), (28009, 1), (50020, 2)], description: "Pak FW/MF/DF/GK 60+ dan 2x EP +500.000." },
    PackageDef { itemid: 50880, name: "Kotak Terima Kasih", icon: "ID50880", price: 5_000_000, contents: &[(50021, 1), (50122, 1), (501, 3)], description: "EP +1.000.000, Kotak Emas dan 3x Lucky EP." },
    PackageDef { itemid: 50881, name: "Kotak World Best", icon: "ID50881", price: 25_000_000, contents: &[(8334, 1), (82214180, 3), (82214068, 5)], description: "World Best 5, 3x World Best 50 2017, 5x World Best 300 2017." },
    PackageDef { itemid: 50882, name: "Kotak Legenda 06-11", icon: "ID50882", price: 15_000_000, contents: &[(45004, 1), (45006, 2), (45007, 3)], description: "06-11 Player Top 10, 2x Top 30, 3x Top 50." },
    PackageDef { itemid: 118012902, name: "Kotak Upgrade", icon: "118012902", price: 20_000_000, contents: &[(15031, 1), (7618, 1), (82232124, 1)], description: "'14 Top 30 +5, Europe '10 Top 30 +3, EC Top 30 +3." },
    PackageDef { itemid: 80000001, name: "VIP Premium", icon: "vipterbaru", price: 10_000_000, contents: &[(50028, 1), (50121, 1), (50021, 5)], description: "Lucky EP max 50 juta, Kotak Platinum, 5x EP +1.000.000." },
    PackageDef { itemid: 81000100, name: "Upgrade Point 100", icon: "8700001", price: 1_000_000, contents: &[(state::ES_ITEM, 100)], description: "100 upgrade point untuk Upgrade." },
    PackageDef { itemid: 81000500, name: "Upgrade Point 500", icon: "8700001", price: 4_500_000, contents: &[(state::ES_ITEM, 500)], description: "500 upgrade point untuk Upgrade." },
    PackageDef { itemid: 81001000, name: "Upgrade Point 1.000", icon: "8700001", price: 8_000_000, contents: &[(state::ES_ITEM, 1000)], description: "1.000 upgrade point untuk Upgrade." },
    // Serial chained packages
    PackageDef { itemid: 70000001, name: "Paket Berantai 1/3", icon: "200130000", price: 500_000, contents: &[(501, 3), (28007, 1)], description: "Tahap 1: 3x Lucky EP dan pak MF 60+." },
    PackageDef { itemid: 70000002, name: "Paket Berantai 2/3", icon: "200130001", price: 2_000_000, contents: &[(50122, 1), (50021, 1)], description: "Tahap 2: Kotak Emas dan EP +1.000.000." },
    PackageDef { itemid: 70000003, name: "Paket Berantai 3/3", icon: "ID50121GP", price: 5_000_000, contents: &[(50121, 1), (8334, 1)], description: "Tahap 3: Kotak Platinum dan World Best 5." },
];

#[derive(Debug, Clone)]
pub struct MonthlyDef {
    pub itemid: u64,
    pub name: &'static str,
    pub icon: &'static str,
    pub days: u32,
    pub price: u64,
    pub now: &'static [(u64, u64)],
    pub daily: &'static [(u64, u64)],
    pub description: &'static str,
}

pub const MONTHLY_PACKS: &[MonthlyDef] = &[
    MonthlyDef {
        itemid: 81000001,
        name: "Paket Bulanan EP",
        icon: "ID50840",
        days: 30,
        price: 2_000_000,
        now: &[(50021, 1)],
        daily: &[(state::EP_ITEM, 200_000)],
        description: "Langsung EP +1.000.000, lalu 200.000 EP tiap hari selama 30 hari.",
    },
    MonthlyDef {
        itemid: 81000002,
        name: "Paket Bulanan Pak",
        icon: "200130001",
        days: 30,
        price: 3_000_000,
        now: &[(8223601, 1)],
        daily: &[(28007, 1), (state::EP_ITEM, 50_000)],
        description: "Langsung pak MF 65+, lalu pak MF 60+ dan 50.000 EP tiap hari selama 30 hari.",
    },
    MonthlyDef {
        itemid: 81000003,
        name: "Paket Bulanan Premium",
        icon: "ID50841",
        days: 30,
        price: 10_000_000,
        now: &[(50121, 1)],
        daily: &[(45013, 1), (state::EP_ITEM, 500_000)],
        description: "Langsung Kotak Platinum, lalu This Season Top 30 dan 500.000 EP tiap hari selama 30 hari.",
    },
];

pub fn icon_val(icon: &str) -> Value {
    if let Ok(n) = icon.parse::<u64>() {
        json!(n)
    } else {
        json!(icon)
    }
}

pub fn get_chain_step(itemid: u64) -> Option<(u64, usize)> {
    for &(first, steps) in CHAINED {
        for (k, &step_id) in steps.iter().enumerate() {
            if step_id == itemid {
                return Some((first, k));
            }
        }
    }
    None
}

pub fn is_chain_step(itemid: u64) -> bool {
    get_chain_step(itemid).is_some()
}

pub fn get_chain_steps(first: u64) -> Option<&'static [u64]> {
    for &(f, steps) in CHAINED {
        if f == first {
            return Some(steps);
        }
    }
    None
}

pub fn product_category(itemid: u64) -> &'static str {
    if state::PACK_DEFS.iter().any(|(id, _, _)| *id == itemid) {
        "PLAYERPACK"
    } else if USE_ITEMS.iter().any(|it| it.itemid == itemid) {
        "USE"
    } else if MONTHLY_PACKS.iter().any(|m| m.itemid == itemid) {
        "MONTHLYPACK"
    } else {
        "PACKAGE"
    }
}

pub fn basic_item_def(itemid: u64) -> Value {
    if itemid == state::EP_ITEM {
        return json!({
            "itemid": state::EP_ITEM,
            "productid": state::EP_ITEM,
            "name": "EP",
            "category": "EP",
            "type": "EP",
            "unit": "EP",
            "itemicon": state::EP_ITEM
        });
    }
    if itemid == state::ES_ITEM {
        return json!({
            "itemid": state::ES_ITEM,
            "productid": state::ES_ITEM,
            "name": "Upgrade point",
            "category": "USE",
            "type": "ES",
            "unit": "COUNT",
            "itemicon": 8700001
        });
    }
    if itemid == state::LR_TICKET {
        return json!({
            "itemid": state::LR_TICKET,
            "productid": state::LR_TICKET,
            "name": "Tiket locker room",
            "category": "LOCKERROOM",
            "type": "LOCKERROOM",
            "unit": "COUNT",
            "itemicon": state::LR_TICKET
        });
    }
    if let Some((name, _)) = state::get_pack_info(itemid) {
        return json!({
            "itemid": itemid,
            "productid": itemid,
            "name": name,
            "category": "PLAYERPACK",
            "type": "CUSTOM",
            "unit": "COUNT",
            "itemicon": itemid
        });
    }
    if let Some(it) = USE_ITEMS.iter().find(|i| i.itemid == itemid) {
        return json!({
            "itemid": itemid,
            "productid": itemid,
            "name": it.name,
            "category": "USE",
            "type": it.item_type,
            "unit": "COUNT",
            "itemicon": icon_val(it.icon)
        });
    }
    json!({
        "itemid": itemid,
        "productid": itemid,
        "name": format!("Item {}", itemid),
        "category": "PLAYERPACK",
        "type": "CUSTOM",
        "unit": "COUNT",
        "itemicon": itemid
    })
}

pub fn item_def(itemid: u64) -> Value {
    if itemid == state::EP_ITEM {
        return json!({
            "itemid": state::EP_ITEM,
            "productid": state::EP_ITEM,
            "name": "EP",
            "category": "EP",
            "type": "EP",
            "unit": "EP",
            "itemicon": state::EP_ITEM,
            "bundleicon": state::EP_ITEM
        });
    }
    if itemid == state::ES_ITEM {
        return json!({
            "itemid": state::ES_ITEM,
            "productid": state::ES_ITEM,
            "name": "Upgrade point",
            "category": "USE",
            "type": "ES",
            "unit": "COUNT",
            "itemicon": 8700001,
            "bundleicon": 8700001
        });
    }
    if itemid == state::LR_TICKET {
        return json!({
            "itemid": state::LR_TICKET,
            "productid": state::LR_TICKET,
            "name": "Tiket locker room",
            "category": "LOCKERROOM",
            "type": "LOCKERROOM",
            "unit": "COUNT",
            "itemicon": state::LR_TICKET,
            "bundleicon": state::LR_TICKET
        });
    }
    if let Some((name, price)) = state::get_pack_info(itemid) {
        return json!({
            "itemid": itemid,
            "productid": itemid,
            "name": name,
            "category": "PLAYERPACK",
            "type": "CUSTOM",
            "unit": "COUNT",
            "itemicon": itemid,
            "bundleicon": itemid,
            "currency": "ep",
            "bunches": [
                {"bunch": 1, "price": price},
                {"bunch": 10, "price": (price as f64 * 9.5) as u64}
            ]
        });
    }
    if let Some(it) = USE_ITEMS.iter().find(|i| i.itemid == itemid) {
        let icon = icon_val(it.icon);
        let mut d = json!({
            "itemid": itemid,
            "productid": itemid,
            "name": it.name,
            "category": "USE",
            "type": it.item_type,
            "unit": "COUNT",
            "itemicon": icon.clone(),
            "bundleicon": icon,
            "currency": "ep",
            "stackable": true,
            "description": it.description,
            "bunches": [
                {"bunch": 1, "price": it.price},
                {"bunch": 10, "price": (it.price as f64 * 9.5) as u64}
            ]
        });
        match it.itemid {
            50121 => {
                d["availables"] = json!([
                    {"itemid": 8334, "count": 1, "staticdata": basic_item_def(8334)},
                    {"itemid": 8194, "count": 1, "staticdata": basic_item_def(8194)},
                    {"itemid": 82214007, "count": 1, "staticdata": basic_item_def(82214007)},
                    {"itemid": 45011, "count": 1, "staticdata": basic_item_def(45011)},
                    {"itemid": state::EP_ITEM, "count": 10_000_000, "staticdata": basic_item_def(state::EP_ITEM)}
                ]);
            }
            50122 => {
                d["availables"] = json!([
                    {"itemid": 82214180, "count": 1, "staticdata": basic_item_def(82214180)},
                    {"itemid": 45012, "count": 1, "staticdata": basic_item_def(45012)},
                    {"itemid": state::EP_ITEM, "count": 3_000_000, "staticdata": basic_item_def(state::EP_ITEM)}
                ]);
            }
            50123 => {
                d["availables"] = json!([
                    {"itemid": 8223600, "count": 1, "staticdata": basic_item_def(8223600)},
                    {"itemid": 8223601, "count": 1, "staticdata": basic_item_def(8223601)},
                    {"itemid": 8223602, "count": 1, "staticdata": basic_item_def(8223602)},
                    {"itemid": state::EP_ITEM, "count": 1_000_000, "staticdata": basic_item_def(state::EP_ITEM)}
                ]);
            }
            50124 => {
                d["availables"] = json!([
                    {"itemid": 28006, "count": 1, "staticdata": basic_item_def(28006)},
                    {"itemid": 28007, "count": 1, "staticdata": basic_item_def(28007)},
                    {"itemid": 28008, "count": 1, "staticdata": basic_item_def(28008)},
                    {"itemid": state::EP_ITEM, "count": 300_000, "staticdata": basic_item_def(state::EP_ITEM)}
                ]);
            }
            50125 => {
                d["availables"] = json!([
                    {"itemid": 82214356, "count": 1, "staticdata": basic_item_def(82214356)},
                    {"itemid": state::EP_ITEM, "count": 100_000, "staticdata": basic_item_def(state::EP_ITEM)}
                ]);
            }
            200130000 => {
                d["availables"] = json!([
                    {"itemid": state::EP_ITEM, "count": 200_000, "staticdata": basic_item_def(state::EP_ITEM)},
                    {"itemid": 28009, "count": 1, "staticdata": basic_item_def(28009)},
                    {"itemid": 82214068, "count": 1, "staticdata": basic_item_def(82214068)}
                ]);
            }
            200130001 => {
                d["availables"] = json!([
                    {"itemid": state::EP_ITEM, "count": 1_000_000, "staticdata": basic_item_def(state::EP_ITEM)},
                    {"itemid": 45007, "count": 1, "staticdata": basic_item_def(45007)},
                    {"itemid": 45013, "count": 1, "staticdata": basic_item_def(45013)}
                ]);
            }
            _ => {}
        }
        return d;
    }
    if let Some(pkg) = PACKAGES.iter().find(|p| p.itemid == itemid) {
        let icon = icon_val(pkg.icon);
        let items: Vec<Value> = pkg
            .contents
            .iter()
            .map(|&(sub_id, sub_count)| {
                json!({
                    "itemid": sub_id,
                    "count": sub_count,
                    "staticdata": basic_item_def(sub_id)
                })
            })
            .collect();

        return json!({
            "itemid": itemid,
            "productid": itemid,
            "name": pkg.name,
            "category": "PACKAGE",
            "type": "ITEM",
            "unit": "COUNT",
            "itemicon": icon.clone(),
            "bundleicon": icon,
            "currency": "ep",
            "description": pkg.description,
            "bunches": [
                {"bunch": 1, "price": pkg.price}
            ],
            "items": items
        });
    }
    if let Some(m) = MONTHLY_PACKS.iter().find(|m| m.itemid == itemid) {
        let icon = icon_val(m.icon);
        let items: Vec<Value> = m
            .now
            .iter()
            .map(|&(sub_id, sub_count)| {
                json!({
                    "itemid": sub_id,
                    "count": sub_count,
                    "staticdata": basic_item_def(sub_id)
                })
            })
            .collect();
        let dailyitems: Vec<Value> = m
            .daily
            .iter()
            .map(|&(sub_id, sub_count)| {
                json!({
                    "itemid": sub_id,
                    "count": sub_count,
                    "staticdata": basic_item_def(sub_id)
                })
            })
            .collect();

        return json!({
            "itemid": itemid,
            "productid": itemid,
            "name": m.name,
            "category": "PACKAGE",
            "type": "ITEM",
            "unit": "COUNT",
            "itemicon": icon.clone(),
            "bundleicon": icon,
            "currency": "ep",
            "description": m.description,
            "daily_period": m.days,
            "monthly_platform": "all",
            "bunches": [
                {"bunch": 1, "price": m.price}
            ],
            "items": items,
            "dailyitems": dailyitems
        });
    }

    json!({
        "itemid": itemid,
        "productid": itemid,
        "name": format!("Item {}", itemid),
        "category": "PLAYERPACK",
        "type": "CUSTOM",
        "unit": "COUNT",
        "itemicon": itemid,
        "bundleicon": itemid
    })
}

pub fn shop_entry(itemid: u64, category: &str, shopid: Option<u64>) -> Value {
    let mut d = item_def(itemid);
    let mut obj = json!({
        "_id": shopid.unwrap_or(itemid).to_string(),
        "shopid": shopid.unwrap_or(itemid),
        "shopcategory": category,
        "forceenable": true,
        "visible": true,
        "salestart": "2014-01-01T00:00:00.000Z",
        "saleend": "2035-01-01T00:00:00.000Z",
    });

    if shopid.is_none() {
        if let Some(pos) = SHOP_NEW.iter().position(|&x| x == itemid) {
            obj["new"] = json!(pos + 1);
        }
        if let Some(pos) = SHOP_BEST.iter().position(|&x| x == itemid) {
            obj["best"] = json!(pos + 1);
        }
    }

    let icon = d.get("bundleicon").cloned().unwrap_or_else(|| json!(itemid));
    let items = if category == "PACKAGE" {
        d.get("items").cloned().unwrap_or_else(|| json!([]))
    } else {
        json!([{"itemid": itemid, "count": 1, "staticdata": d.clone()}])
    };

    if let Some(map) = d.as_object_mut() {
        map.insert("bundleicon".to_string(), icon.clone());
        map.insert("items".to_string(), items);
    }

    obj["product"] = json!({
        "itemid": itemid,
        "staticdata": d
    });

    obj
}

pub fn chained_entry(st: &Value, first: u64) -> Value {
    let fallback = [first];
    let steps: &[u64] = get_chain_steps(first).unwrap_or(&fallback);
    let coach = state::my_coach(st);
    let done = coach
        .get("chain")
        .and_then(|v| v.get(&first.to_string()))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    let mut parts: Vec<Value> = steps
        .iter()
        .enumerate()
        .map(|(k, &i)| {
            let mut e = shop_entry(i, "CHAINED", Some(i));
            let is_bought = k < done;
            e["remainedperaccount"] = json!(if is_bought { 0 } else { 1 });
            e["buy"] = json!(is_bought);
            e
        })
        .collect();

    if parts.is_empty() {
        return shop_entry(first, "CHAINED", Some(first));
    }

    let mut first_entry = parts.remove(0);
    first_entry["step"] = json!(done.min(steps.len() - 1));
    first_entry["chained"] = Value::Array(parts);
    first_entry
}

pub fn shop_list(st: &Value) -> Vec<Value> {
    let mut list = Vec::new();
    // 1. PLAYERPACK
    for &(id, _, _) in state::PACK_DEFS {
        list.push(shop_entry(id, "PLAYERPACK", None));
    }
    // 2. USE
    for item in USE_ITEMS {
        list.push(shop_entry(item.itemid, "USE", None));
    }
    // 3. PACKAGE (not in serial chained steps)
    for pkg in PACKAGES {
        if !is_chain_step(pkg.itemid) {
            list.push(shop_entry(pkg.itemid, "PACKAGE", None));
        }
    }
    // 4. MILEAGE (MP_SHOP items: shopid = MP_SHOPID + id)
    for &id in MP_SHOP {
        list.push(shop_entry(id, "MILEAGE", Some(MP_SHOPID + id)));
    }
    // 5. MONTHLYPACK
    for m in MONTHLY_PACKS {
        list.push(shop_entry(m.itemid, "MONTHLYPACK", None));
    }
    // 6. CHAINED
    for &(first, _) in CHAINED {
        list.push(chained_entry(st, first));
    }
    list
}

pub fn stshop_list() -> Vec<Value> {
    SPECIAL
        .iter()
        .map(|&id| {
            let cat = product_category(id);
            let mut entry = shop_entry(id, cat, Some(STSHOPID + id));
            if let Some(obj) = entry.as_object_mut() {
                obj.insert("stid".to_string(), json!(STSHOPID + id));
            }
            entry
        })
        .collect()
}

pub fn gpshop_list() -> Vec<Value> {
    GPSHOP_ITEMS
        .iter()
        .map(|&id| {
            shop_entry(id, "GPSHOP", Some(GPSHOPID + id))
        })
        .collect()
}

pub fn shop_remained_cash() -> Value {
    json!({
        "result": true,
        "remainedcash": 0
    })
}

pub fn specific_target() -> Value {
    json!({
        "issell": true,
        "targets": []
    })
}

pub fn monthlypack_list(st: &Value) -> Vec<Value> {
    let now_sec = Utc::now().timestamp();
    let coach = state::my_coach(st);
    let mut list = Vec::new();

    if let Some(monthly_map) = coach.get("monthly").and_then(|v| v.as_object()) {
        for (k, v) in monthly_map {
            let exp = v.get("expire").and_then(|x| x.as_i64()).unwrap_or(0);
            if exp > now_sec {
                if let Ok(id) = k.parse::<u64>() {
                    let since = v.get("since").and_then(|x| x.as_i64()).unwrap_or(now_sec);
                    let rewarded = v.get("rewarded").and_then(|x| x.as_i64()).unwrap_or(0);

                    let since_str = DateTime::from_timestamp(since, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_else(|| "2026-01-01T00:00:00Z".to_string());
                    let expire_str = DateTime::from_timestamp(exp, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_else(|| "2026-01-01T00:00:00Z".to_string());
                    let rewarded_str = if rewarded > 0 {
                        DateTime::from_timestamp(rewarded, 0)
                            .map(|dt| dt.to_rfc3339())
                            .unwrap_or_else(|| "2026-01-01T00:00:00Z".to_string())
                    } else {
                        "2026-01-01T00:00:00Z".to_string()
                    };

                    list.push(json!({
                        "_id": format!("{:024x}", id),
                        "productid": id,
                        "product": {
                            "itemid": id,
                            "staticdata": item_def(id)
                        },
                        "purchasedat": since_str,
                        "expireat": expire_str,
                        "rewardedat": rewarded_str
                    }));
                }
            }
        }
    }

    list
}

pub fn claim_monthly_rewards(st: &mut Value, _slot: usize) {
    let now_sec = Utc::now().timestamp();
    let today = state::today_wib(Some(now_sec));

    let coach = state::my_coach_mut(st);
    let monthly_map = if let Some(m) = coach.get_mut("monthly").and_then(|v| v.as_object_mut()) {
        m.clone()
    } else {
        return;
    };

    let mut to_update = Vec::new();
    let mut rewards_to_add = Vec::new();

    for (k, v) in &monthly_map {
        let exp = v.get("expire").and_then(|x| x.as_i64()).unwrap_or(0);
        let rewarded_ts = v.get("rewarded").and_then(|x| x.as_i64()).unwrap_or(0);
        let rewarded_day = if rewarded_ts > 0 {
            state::today_wib(Some(rewarded_ts))
        } else {
            String::new()
        };

        if exp > now_sec && rewarded_day != today {
            if let Ok(id) = k.parse::<u64>() {
                if let Some(mdef) = MONTHLY_PACKS.iter().find(|m| m.itemid == id) {
                    to_update.push(k.clone());
                    let items: Vec<Value> = mdef
                        .daily
                        .iter()
                        .map(|&(itemid, count)| json!({"itemid": itemid, "count": count}))
                        .collect();
                    rewards_to_add.push((mdef.name, items));
                }
            }
        }
    }

    let coach = state::my_coach_mut(st);
    if let Some(m) = coach.get_mut("monthly").and_then(|v| v.as_object_mut()) {
        for k in to_update {
            if let Some(entry) = m.get_mut(&k).and_then(|v| v.as_object_mut()) {
                entry.insert("rewarded".to_string(), json!(now_sec));
            }
        }
    }

    for (name, items) in rewards_to_add {
        let msg = format!("Hadiah harian {}.", name);
        let bag_entry = json!({
            "_id": state::new_iid(),
            "id": state::new_iid(),
            "type": "monthlypack",
            "title": format!("{} - hadiah harian", name),
            "msg": msg,
            "date": Utc::now().to_rfc3339(),
            "items": items,
            "players": [],
            "bundleicon": "monthlypack"
        });
        let coach = state::my_coach_mut(st);
        if coach.get("bag").is_none() {
            coach["bag"] = json!([]);
        }
        if let Some(bag) = coach.get_mut("bag").and_then(|v| v.as_array_mut()) {
            bag.push(bag_entry);
        }
    }
}

pub struct BunchOption {
    pub bunch: u64,
    pub price: u64,
}

pub fn get_bunches(itemid: u64) -> Vec<BunchOption> {
    if let Some((_, price)) = state::get_pack_info(itemid) {
        return vec![
            BunchOption { bunch: 1, price },
            BunchOption { bunch: 10, price: (price as f64 * 9.5) as u64 },
        ];
    }
    if let Some(it) = USE_ITEMS.iter().find(|i| i.itemid == itemid) {
        return vec![
            BunchOption { bunch: 1, price: it.price },
            BunchOption { bunch: 10, price: (it.price as f64 * 9.5) as u64 },
        ];
    }
    if let Some(pkg) = PACKAGES.iter().find(|p| p.itemid == itemid) {
        return vec![BunchOption { bunch: 1, price: pkg.price }];
    }
    if let Some(m) = MONTHLY_PACKS.iter().find(|m| m.itemid == itemid) {
        return vec![BunchOption { bunch: 1, price: m.price }];
    }
    vec![]
}

pub fn get_package_contents(itemid: u64) -> Option<Vec<(u64, u64)>> {
    if let Some(pkg) = PACKAGES.iter().find(|p| p.itemid == itemid) {
        return Some(pkg.contents.to_vec());
    }
    if let Some(m) = MONTHLY_PACKS.iter().find(|m| m.itemid == itemid) {
        return Some(m.now.to_vec());
    }
    None
}

pub fn handle_shop_purchase<F>(body: &[u8], build_coach_me: F) -> Value
where
    F: FnOnce(&mut Value, usize) -> Value,
{
    let req: Value = serde_json::from_slice(body).unwrap_or(json!({}));
    let mut itemid = req.get("productid").and_then(|v| v.as_u64()).unwrap_or(0);
    if itemid == 0 {
        let sid = req.get("shopid").and_then(|v| v.as_u64()).unwrap_or(0);
        if sid >= GPSHOPID {
            itemid = sid - GPSHOPID;
        } else if sid >= STSHOPID {
            itemid = sid - STSHOPID;
        } else if sid >= MP_SHOPID {
            itemid = sid - MP_SHOPID;
        } else {
            itemid = sid;
        }
    }

    let bunches = get_bunches(itemid);
    if bunches.is_empty() {
        return json!({"result": false, "reason": "NOT_EXIST_PRODUCT"});
    }

    let bunch_idx = req.get("bunch").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let b = if bunch_idx < bunches.len() {
        &bunches[bunch_idx]
    } else {
        &bunches[bunches.len() - 1]
    };

    let mut st = state::load_state();
    let slot = state::default_slot(&st);
    let coach = state::my_coach(&st);
    let current_ep = coach.get("ep").and_then(|v| v.as_i64()).unwrap_or(state::START_EP);
    if current_ep < b.price as i64 {
        return json!({"result": false, "reason": "NOT_ENOUGH_EP"});
    }

    // Deduct EP
    state::add_ep(&mut st, -(b.price as i64));

    // Deliver product contents
    let cat = product_category(itemid);
    if let Some(contents) = get_package_contents(itemid) {
        for &(sub_id, sub_count) in &contents {
            let total = sub_count * b.bunch;
            if sub_id == state::EP_ITEM {
                state::add_ep(&mut st, total as i64);
            } else if sub_id == state::ES_ITEM {
                state::add_es(&mut st, total as i64);
            } else {
                state::inventory_add(&mut st, sub_id, total);
            }
        }
    } else {
        state::inventory_add(&mut st, itemid, b.bunch);
    }

    // Monthly pack extension
    if cat == "MONTHLYPACK" {
        if let Some(mdef) = MONTHLY_PACKS.iter().find(|m| m.itemid == itemid) {
            let now_sec = Utc::now().timestamp();
            let coach = state::my_coach_mut(&mut st);
            if coach.get("monthly").is_none() {
                coach["monthly"] = json!({});
            }
            let key = itemid.to_string();
            let cur_exp = coach["monthly"]
                .get(&key)
                .and_then(|m| m.get("expire"))
                .and_then(|x| x.as_i64())
                .unwrap_or(now_sec);
            let new_exp = cur_exp.max(now_sec) + (mdef.days as i64) * 86400;
            coach["monthly"][key] = json!({
                "since": now_sec,
                "expire": new_exp,
                "rewarded": 0
            });
        }
    }

    // Chained step progression
    if let Some((first, k)) = get_chain_step(itemid) {
        let coach = state::my_coach_mut(&mut st);
        if coach.get("chain").is_none() {
            coach["chain"] = json!({});
        }
        let key = first.to_string();
        let cur_done = coach["chain"].get(&key).and_then(|x| x.as_u64()).unwrap_or(0);
        coach["chain"][key] = json!(cur_done.max((k + 1) as u64));
    }

    let _ = state::save_state(&st);
    let coach_me_val = build_coach_me(&mut st, slot);

    let mut resources = vec![
        json!({"uri": "/coach/me", "body": coach_me_val}),
    ];
    if cat == "MONTHLYPACK" {
        resources.push(json!({"uri": "/monthlypack", "body": monthlypack_list(&st)}));
    }

    json!({
        "result": true,
        "resources": resources
    })
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_shop_list_categories() {
        let st = json!({
            "coaches": [{"slot": 0, "name": "Manager", "chain": {}, "monthly": {}}]
        });
        let list = shop_list(&st);
        assert!(!list.is_empty(), "Shop list must not be empty");

        let categories: Vec<String> = list
            .iter()
            .filter_map(|e| e.get("shopcategory").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect();

        assert!(categories.contains(&"PLAYERPACK".to_string()));
        assert!(categories.contains(&"USE".to_string()));
        assert!(categories.contains(&"PACKAGE".to_string()));
        assert!(categories.contains(&"MILEAGE".to_string()));
        assert!(categories.contains(&"MONTHLYPACK".to_string()));
        assert!(categories.contains(&"CHAINED".to_string()));
    }

    #[test]
    fn test_shop_entry_structure() {
        let entry = shop_entry(28001, "PLAYERPACK", None);
        assert_eq!(entry.get("_id").and_then(|v| v.as_str()), Some("28001"));
        assert_eq!(entry.get("shopid").and_then(|v| v.as_u64()), Some(28001));
        assert_eq!(entry.get("shopcategory").and_then(|v| v.as_str()), Some("PLAYERPACK"));
        assert_eq!(entry.get("forceenable").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(entry.get("visible").and_then(|v| v.as_bool()), Some(true));
        let product = entry.get("product").expect("product field required");
        assert_eq!(product.get("itemid").and_then(|v| v.as_u64()), Some(28001));
        let staticdata = product.get("staticdata").expect("staticdata field required");
        assert_eq!(staticdata.get("name").and_then(|v| v.as_str()), Some("Premier League 65+"));
        assert!(staticdata.get("bunches").and_then(|v| v.as_array()).is_some());
    }

    #[test]
    fn test_stshop_list() {
        let stshop = stshop_list();
        assert_eq!(stshop.len(), SPECIAL.len());
        for entry in &stshop {
            let shopid = entry.get("shopid").and_then(|v| v.as_u64()).unwrap();
            assert!(shopid >= STSHOPID);
            assert_eq!(entry.get("stid").and_then(|v| v.as_u64()), Some(shopid));
            assert_eq!(entry.get("forceenable").and_then(|v| v.as_bool()), Some(true));
            assert_eq!(entry.get("visible").and_then(|v| v.as_bool()), Some(true));
        }
    }

    #[test]
    fn test_gpshop_list() {
        let gpshop = gpshop_list();
        assert_eq!(gpshop.len(), GPSHOP_ITEMS.len());
        for entry in &gpshop {
            let shopid = entry.get("shopid").and_then(|v| v.as_u64()).unwrap();
            assert!(shopid >= GPSHOPID);
            assert_eq!(entry.get("shopcategory").and_then(|v| v.as_str()), Some("GPSHOP"));
            assert_eq!(entry.get("forceenable").and_then(|v| v.as_bool()), Some(true));
            assert_eq!(entry.get("visible").and_then(|v| v.as_bool()), Some(true));
        }
    }

    #[test]
    fn test_shop_remained_cash() {
        let res = shop_remained_cash();
        assert_eq!(res.get("remainedcash").and_then(|v| v.as_u64()), Some(0));
    }

    #[test]
    fn test_specific_target() {
        let res = specific_target();
        assert_eq!(res.get("issell").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(res.get("targets").and_then(|v| v.as_array()).map(|a| a.len()), Some(0));
    }

    #[test]
    fn test_monthlypack_empty() {
        let st = json!({
            "coaches": [{"slot": 0, "name": "Manager", "monthly": {}}]
        });
        let list = monthlypack_list(&st);
        assert!(list.is_empty());
    }

    #[test]
    fn test_shop_purchase_flow() {
        let _lock = state::STATE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let mut st = state::load_state();
        let coach = state::my_coach_mut(&mut st);
        coach["ep"] = json!(100_000_000);
        let _ = state::save_state(&st);
        // 1. Purchase player pack (bunch 0)
        let buy_body = serde_json::to_vec(&json!({
            "productid": 28001,
            "shopid": 28001,
            "bunch": 0
        })).unwrap();
        let res = handle_shop_purchase(&buy_body, |_st, _slot| json!({"coach_me_called": true}));
        assert_eq!(res.get("result").and_then(|v| v.as_bool()), Some(true));
        let res_arr = res.get("resources").and_then(|v| v.as_array()).unwrap();
        assert_eq!(res_arr[0].get("uri").and_then(|v| v.as_str()), Some("/coach/me"));

        // 2. Insufficient EP
        let mut st = state::load_state();
        let coach = state::my_coach_mut(&mut st);
        coach["ep"] = json!(0);
        let _ = state::save_state(&st);
        let res_fail = handle_shop_purchase(&buy_body, |_st, _slot| json!({}));
        assert_eq!(res_fail.get("result").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(res_fail.get("reason").and_then(|v| v.as_str()), Some("NOT_ENOUGH_EP"));

        // 3. Invalid product
        let invalid_body = serde_json::to_vec(&json!({
            "productid": 99999999,
            "bunch": 0
        })).unwrap();
        let res_invalid = handle_shop_purchase(&invalid_body, |_st, _slot| json!({}));
        assert_eq!(res_invalid.get("result").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(res_invalid.get("reason").and_then(|v| v.as_str()), Some("NOT_EXIST_PRODUCT"));

        // Restore EP
        let mut st = state::load_state();
        let coach = state::my_coach_mut(&mut st);
        coach["ep"] = json!(100_000_000);
        let _ = state::save_state(&st);

        // 4. Purchase package (801 Paket Starter)
        let pkg_body = serde_json::to_vec(&json!({
            "productid": 801,
            "bunch": 0
        })).unwrap();
        let res_pkg = handle_shop_purchase(&pkg_body, |_st, _slot| json!({}));
        assert_eq!(res_pkg.get("result").and_then(|v| v.as_bool()), Some(true));

        // 5. Purchase chained package step 1 (70000001)
        let chain_body = serde_json::to_vec(&json!({
            "productid": 70000001,
            "bunch": 0
        })).unwrap();
        let res_chain = handle_shop_purchase(&chain_body, |_st, _slot| json!({}));
        assert_eq!(res_chain.get("result").and_then(|v| v.as_bool()), Some(true));
        let st_chain = state::load_state();
        let chained_item = chained_entry(&st_chain, 70000001);
        assert_eq!(chained_item.get("step").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(chained_item.get("remainedperaccount").and_then(|v| v.as_u64()), Some(0));

        // 6. Purchase monthly package (81000001)
        let monthly_body = serde_json::to_vec(&json!({
            "productid": 81000001,
            "bunch": 0
        })).unwrap();
        let res_monthly = handle_shop_purchase(&monthly_body, |_st, _slot| json!({}));
        assert_eq!(res_monthly.get("result").and_then(|v| v.as_bool()), Some(true));
        let st_monthly = state::load_state();
        let mlist = monthlypack_list(&st_monthly);
        assert!(!mlist.is_empty(), "Monthly pack list should have active package");
    }
}
