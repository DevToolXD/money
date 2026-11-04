//! 예전 JS(app/shapes.js, app/biz.js)와 같은 글자가 나오는지 비교하려고 모든 그림을 한 줄씩 적어요.
//! 쓰는 법: tests/goods-same.sh
use goods::catalog::{cost_lines, name_of, pkv_html, thumb_html, Pick};
use goods::shapes::{self, body, cap, num};

fn main() {
    let fams: [(&str, Vec<&str>); 7] = [
        ("bottle", shapes::BOTTLE_KINDS.to_vec()),
        ("bag", [shapes::BAG_CHIPS, shapes::BAG_JELLY, shapes::BAG_BREAD].concat()),
        ("box", shapes::BOX_KINDS.to_vec()),
        ("meat", shapes::MEAT_KINDS.to_vec()),
        ("can", shapes::CAN_KINDS.to_vec()),
        ("cup", shapes::CUP_KINDS.to_vec()),
        ("wrap", shapes::WRAP_KINDS.to_vec()),
    ];
    for (fam, ks) in &fams {
        for k in ks.iter().chain(["nope"].iter()) {
            for s in 0..5 {
                let r = body(fam, k, s);
                println!("B|{fam}|{k}|{s}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}", r.body, r.ext, r.det, r.hl, r.win, r.xf, num(r.cy), num(r.half), num(r.top), num(r.nr));
                println!("T|{fam}|{k}|{s}|{}", thumb_html(fam, k, s));
            }
        }
    }
    for k in shapes::CAP_KINDS.iter().chain(["nope"].iter()) {
        for s in 0..5 {
            for (nr, t) in [(12.0, 22.0), (10.4, 30.7), (9.0, 18.5)] {
                let (b, d) = cap(k, nr, t, s);
                println!("C|{k}|{s}|{}|{}|{b}|{d}", num(nr), num(t));
            }
            println!("T|cap|{k}|{s}|{}", thumb_html("cap", k, s));
        }
    }
    let items = ["water", "jelly", "chips", "bread", "lunchbox", "meat", "bar", "drink", "coffee"];
    for item in items {
        for shape in [None, Some(0), Some(7), Some(23), Some(49)] {
            for (cap, cc, color, content) in [(None, None, None, None), (Some(13), Some(42), Some(5), Some(2)), (Some(49), None, Some(49), Some(5))] {
                let p = Pick { item: item.into(), shape, cap, cap_color: cc, color, content };
                let (lines, total) = cost_lines(&p);
                let ls: Vec<String> = lines.iter().map(|(n, v)| format!("{n}={v}")).collect();
                println!("P|{item}|{shape:?}|{cap:?}|{cc:?}|{color:?}|{content:?}|{}|{total}|{}", ls.join(","), name_of(&p));
                println!("V|{item}|{shape:?}|{cap:?}|{cc:?}|{color:?}|{content:?}|{}", pkv_html(&p, shape == Some(7), "<b>사</b>", false));
            }
        }
    }
}
