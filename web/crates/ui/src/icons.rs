//! 탭 아이콘. 28칸 격자 안에 그린 외곽선 아이콘이에요 (예전 app/icons.js에서 그대로 옮겼어요).
//! 선 굵기는 모두 같고, 끝과 꺾임은 둥글어요. 캔버스(Path2D)와 DOM(svg) 양쪽에서 같은 데이터를 써요.

pub const SIZE: f64 = 28.0;

#[derive(Clone, Copy)]
pub struct Part {
    pub d: &'static str,
    pub stroke: f64,
    pub fill: bool,
    pub alpha: f64,
    pub evenodd: bool,
    pub transform: Option<[f64; 6]>,
}

const MESSAGES: &[Part] = &[
    Part { d: "M2 12.31 26 2 15.66 26 11.13 16.88Z", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M11.13 16.88 26 2", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
];
const MONEY: &[Part] = &[
    Part { d: "M6 8.6V6.4A2.9 2.9 0 0 1 8.9 3.5H20.6A2.9 2.9 0 0 1 23.5 6.4V8.6", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M6.2 8.6H21.8A3.7 3.7 0 0 1 25.5 12.3V19.9A3.7 3.7 0 0 1 21.8 23.6H6.2A3.7 3.7 0 0 1 2.5 19.9V12.3A3.7 3.7 0 0 1 6.2 8.6Z", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M25.5 13.6H19.9A2.9 2.9 0 0 0 19.9 19.4H25.5", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M19.9 16.5h.01", stroke: 2.1, fill: false, alpha: 1.0, evenodd: false, transform: None },
];
const WORK: &[Part] = &[
    Part { d: "M 212.5 177 C 202.6 183.5 188 193.1 180 198.3 C 172 203.5 159.9 211.5 153 215.9 C 146.1 220.4 140.5 224.5 140.5 225 C 140.5 226 230.7 285 232.2 285 C 232.7 285 233 278.1 232.8 269 L 232.5 252.9 275.5 253.2 C 317.8 253.5 318.6 253.5 324.2 255.8 C 358.6 269.7 370 310.5 347.4 339.2 C 342.5 345.5 342.5 346.3 347.3 350.9 L 349.9 353.2 353.8 348.9 C 359.2 343.1 363.6 335.6 366.8 326.8 C 370.7 316 370.7 296 366.8 285.2 C 360.1 266.7 347 253.2 328.7 245.8 L 320.5 242.5 270.8 242.2 L 221.1 241.9 220.8 253.4 L 220.5 264.8 190.3 245.2 C 173.7 234.3 160.1 225.3 160.1 225 C 160.1 224.7 173.7 215.7 190.3 204.8 L 220.5 185.2 220.8 196.6 L 221.1 208 254 208 L 287 208 287.5 205.8 C 288 203.7 291 183.6 291 182.5 C 291 182.2 297.8 182 306 182 C 314.3 182 321 182.2 321 182.5 C 321 183.6 324 203.6 324.5 205.6 C 324.8 207.1 326.2 207.9 329.9 208.8 C 337.4 210.5 349.9 215.6 356.4 219.5 C 359.6 221.4 362.7 222.7 363.4 222.3 C 364 221.9 368.7 218.5 373.8 214.6 L 383.1 207.7 393.7 218.2 L 404.3 228.8 397.3 238.1 C 393.5 243.3 390.1 248 389.7 248.6 C 389.4 249.2 391 253.2 393.4 257.4 C 396.9 263.5 402 276.9 404.5 286.6 C 404.7 287.1 409.2 288.2 414.6 288.9 C 420.1 289.6 425.7 290.4 427.3 290.7 L 430 291.3 430 306.1 C 430 314.3 429.8 321 429.5 321 C 428.4 321 408.6 324.1 408.4 324.3 C 408.1 324.6 409.9 334.2 410.3 334.7 C 410.5 334.9 417.6 334 426.1 332.8 L 441.5 330.5 441.5 306 L 441.5 281.5 427.7 279.5 L 413.9 277.5 411.1 269.5 C 409.6 265.1 407.1 259.2 405.7 256.3 C 404.2 253.4 403 250.6 403 250 C 403 249.3 406.6 244.1 411.1 238.3 L 419.2 227.7 401.7 210.2 L 384.3 192.8 373.7 200.9 C 367.8 205.4 362.6 209 362 209 C 361.4 209 358.6 207.8 355.7 206.3 C 352.8 204.9 346.9 202.5 342.5 201 L 334.5 198.2 332.5 184.4 L 330.5 170.5 306 170.5 L 281.5 170.5 279.6 183.8 L 277.7 197 255.1 197 L 232.5 197 232.8 181 C 233 168.4 232.8 165 231.8 165.1 C 231.1 165.1 222.4 170.4 212.5 177 M 255 266.8 C 253.3 268.8 250.4 273.7 248.5 277.5 C 233 309.5 246.1 348.3 277.5 363.5 C 290.4 369.8 292.4 370 344.2 370 L 390.9 370 391.2 358.6 L 391.5 347.2 421.7 366.9 C 438.3 377.7 451.9 386.7 451.9 387 C 452 387.3 438.4 396.4 421.7 407.2 L 391.5 426.9 391.2 415.4 L 390.9 404 358 404 C 321.2 404 324.7 403.2 323.6 411.5 C 323.3 413.7 322.6 418.8 322 422.8 L 320.8 430 306 430 L 291.3 430 290.7 427.3 C 290.4 425.7 289.6 420.1 288.9 414.6 C 288.2 409.2 287.1 404.7 286.6 404.5 C 276.9 402 263.5 396.9 257.4 393.4 C 253.2 391 249.2 389.4 248.6 389.7 C 248 390.1 243.3 393.5 238.1 397.3 L 228.8 404.3 218.2 393.7 L 207.7 383.1 214.6 373.8 C 218.5 368.7 221.9 364 222.3 363.4 C 222.7 362.7 221.4 359.6 219.5 356.4 C 215.6 349.9 210.5 337.4 208.8 329.9 C 207.9 326.2 207.1 324.8 205.6 324.5 C 203.6 324 183.6 321 182.5 321 C 182.2 321 182 314.3 182 306.1 L 182 291.2 189.3 290 C 193.2 289.4 198.1 288.7 200.1 288.4 L 203.7 287.9 202.9 282.8 C 202.4 280 201.9 277.6 201.7 277.4 C 201.5 277.2 194.4 278 185.9 279.2 L 170.5 281.5 170.5 306 L 170.5 330.5 184.4 332.5 L 198.3 334.5 200.5 341.2 C 201.7 344.9 204.2 350.9 205.9 354.5 C 207.6 358.2 209 361.6 209 362.1 C 209 362.5 205.4 367.7 200.9 373.6 L 192.8 384.3 210.2 401.7 L 227.7 419.2 238.3 411.1 C 244.2 406.6 249.5 403 250 403 C 250.6 403 253.4 404.2 256.3 405.7 C 259.2 407.1 265.1 409.6 269.5 411.1 L 277.5 413.9 279.5 427.7 L 281.5 441.5 306 441.5 L 330.5 441.5 332.4 428.3 L 334.3 415 356.7 415 L 379 415 379 431 C 379 440.2 379.4 447 379.9 447 C 380.8 447 391.1 440.4 429 415.7 C 436.4 410.8 449 402.6 457 397.4 C 465 392.2 471.5 387.6 471.5 387.1 C 471.5 386.1 381.5 327 380 327 C 379.4 327 379 333.5 379 343 L 379 359 339.5 359 C 314 359 298.2 358.6 294.8 357.9 C 265.6 351.6 247.6 322.7 254.5 293.2 C 256.2 286.2 257.6 283 262.8 275.2 L 266.3 269.9 262.9 266.4 C 258.8 262.2 258.8 262.2 255 266.8", stroke: 9.548, fill: true, alpha: 1.0, evenodd: true, transform: Some([0.07855, 0.0, 0.0, 0.07855, -10.0363, -10.0402]) },
];
const UNTITLED: &[Part] = &[
    Part { d: "M9.2 10.3A4.8 4.8 0 1 1 15.7 14.7C14.5 15.5 14 16.3 14 17.9", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M14 23.2h.01", stroke: 2.3, fill: false, alpha: 1.0, evenodd: false, transform: None },
];
const INVENTORY: &[Part] = &[
    Part { d: "M7 12a7 7 0 0 1 14 0v10.500a1.500 1.500 0 0 1-1.500 1.500h-11A1.500 1.500 0 0 1 7 22.500Z", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M11 5.700V4.500a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v1.200", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M7 14.500h14", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M10.500 18h7v3.500h-7Z", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
];
const SHOP: &[Part] = &[
    Part { d: "M2.5 4.5h3.6l2.6 12.4a2 2 0 0 0 2 1.6h9.4a2 2 0 0 0 2-1.5l1.9-8.2H7.2", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M11 23.2a1.7 1.7 0 1 0 3.4 0 1.7 1.7 0 1 0-3.4 0Z", stroke: 0.0, fill: true, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M20 23.2a1.7 1.7 0 1 0 3.4 0 1.7 1.7 0 1 0-3.4 0Z", stroke: 0.0, fill: true, alpha: 1.0, evenodd: false, transform: None },
];
const RANKING: &[Part] = &[
    Part { d: "M3.5 24.5V15h6.5v9.5", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M10 24.5V4.5h8v20", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M18 24.5V11h6.5v13.500", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M2 24.5h24", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
];
const MANAGE: &[Part] = &[
    Part { d: "M4 8h9.500M18.500 8H24", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M16 5.500a2.500 2.500 0 1 1 0 5 2.500 2.500 0 0 1 0-5Z", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M4 14h3.500M12.500 14H24", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M10 11.500a2.500 2.500 0 1 1 0 5 2.500 2.500 0 0 1 0-5Z", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M4 20h11M20.500 20H24", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M18 17.500a2.500 2.500 0 1 1 0 5 2.500 2.500 0 0 1 0-5Z", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
];
const BUSINESS: &[Part] = &[
    Part { d: "M5 24.500V5.500a1 1 0 0 1 1-1h5.500a1 1 0 0 1 1 1V24.500", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M15.500 24.500V5.500a1 1 0 0 1 1-1H22a1 1 0 0 1 1 1V24.500", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M2.500 24.500h23", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M8.750 8.500V10M8.750 12.500V14M8.750 16.500V18M19.250 8.500V10M19.250 12.500V14M19.250 16.500V18", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
];
const ACCOUNT: &[Part] = &[
    Part { d: "M14 0.8A13.2 13.2 0 1 1 14 27.2 13.2 13.2 0 0 1 14 0.8Z", stroke: 0.0, fill: true, alpha: 0.13, evenodd: false, transform: None },
    Part { d: "M14 4.6A5.6 5.6 0 1 1 14 15.8 5.6 5.6 0 0 1 14 4.6Z", stroke: 0.0, fill: true, alpha: 0.42, evenodd: false, transform: None },
    Part { d: "M3.6 23.2C4.8 18.6 9 16.6 14 16.6S23.2 18.6 24.4 23.2A13.1 13.1 0 0 1 3.6 23.2Z", stroke: 0.0, fill: true, alpha: 0.42, evenodd: false, transform: None },
];

/// 탭 이름 → 아이콘
// SNS: 두 사람 (앞사람이 크고, 뒷사람은 오른쪽 뒤에 살짝 가려져요)
const SNS: &[Part] = &[
    Part { d: "M10 4.6A4.3 4.3 0 1 1 10 13.2 4.3 4.3 0 0 1 10 4.6Z", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M2.5 24.2C2.5 19.6 5.9 16.4 10 16.4S17.5 19.6 17.5 24.2Z", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M15.6 8.4A3.6 3.6 0 1 1 18.9 13.4", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
    Part { d: "M17.4 17.1C18.2 16.7 19.1 16.5 20 16.5 23.1 16.5 25.5 19.2 25.5 23.3V24.2H19.6", stroke: 1.5, fill: false, alpha: 1.0, evenodd: false, transform: None },
];

pub fn icon(name: &str) -> Option<&'static [Part]> {
    Some(match name {
        "messages" => MESSAGES,
        "money" => MONEY,
        "work" => WORK,
        "untitled" => UNTITLED,
        "inventory" => INVENTORY,
        "untitled3" => SNS,
        "shop" => SHOP,
        "ranking" => RANKING,
        "manage" => MANAGE,
        "business" => BUSINESS,
        "account" => ACCOUNT,
        _ => return None,
    })
}
