extern crate serde_json;

use serde::{Deserialize, Serialize};

use ansi_term::Color;
use std::fmt::{Display, Formatter, Error};
use reqwest;
use curl::easy::{Easy, List};
use std::io::Read;
use serde::export::fmt::Debug;
use std::cmp::max;
use rand::{thread_rng, Rng};
use std::time::SystemTime;
use PlayerColor::*;

const TOKEN: &str = env!("taketoken");

const A1: [&str; 8] = ["a", "b", "c", "d", "e", "f", "g", "h"];
const A2: [&str; 8] = ["1", "2", "3", "4", "5", "6", "7", "8"];

#[derive(Copy, Clone, PartialEq)]
enum PlayerColor {
    White,
    Black,
    Unknown
}
impl PlayerColor {
    fn color(&self) -> Color {
        match self {
            White => Color::Fixed(232),
            Black => Color::Fixed(255),
            Unknown => Color::RGB(42, 42, 42)
        }
    }
}

const ENDPOINT_BASE: &str = "https://lichess.org";

#[allow(unused)]
const ENDPOINT_PROFILE: &str = "/api/account";
#[allow(unused)]
const ENDPOINT_PLAYING: &str = "/api/account/playing";
const ENDPOINT_STREAM: &str = "/api/bot/game/stream/{}";
const ENDPOINT_STREAM_EVENT: &str = "/api/stream/event";
#[allow(unused)]
const ENDPOINT_GAME: &str = "/api/bot/game/{}";
const ENDPOINT_MOVE: &str = "/api/bot/game/{}/move/{}";
const ENDPOINT_CHAT: &str = "/api/bot/game/{}/chat";
#[allow(unused)]
const ENDPOINT_ABORT: &str = "/api/bot/game/{}/abort";
#[allow(unused)]
const ENDPOINT_CHALLENGE: &str = "/api/challenge/{}";
const ENDPOINT_ACCEPT: &str = "/api/challenge/{}/accept";
#[allow(unused)]
const ENDPOINT_DECLINE: &str = "/api/challenge/{}/decline";
#[allow(unused)]
const ENDPOINT_UPGRADE: &str = "/api/bot/account/upgrade";
const ENDPOINT_RESIGN: &str = "/api/bot/game/{}/resign";

const ENDPOINT_TABLEBASE: &str = "http://tablebase.lichess.ovh/standard?fen=";

fn main() {
    let bot = Bot::new();
    bot.setup_events();
}

fn get_tablebase_move(table: &Table, color: PlayerColor) -> Option<Move> {
    let res = reqwest::blocking::get(&format!("{}{}", ENDPOINT_TABLEBASE, table.fen(color))).unwrap().text().unwrap();
    if let Ok(res) = serde_json::from_str::<TablebaseResponse>(&res) {
        Some(Move::from_str(&*res.moves[0].uci))
    } else {
        None
    }
}

#[derive(Serialize)]
struct Challenge {
    rated: bool,
    #[serde(rename = "clock.limit")]
    clocklimit: i32,
    #[serde(rename = "clock.increment")]
    clockincrement: i32,
    color: String,
    variant: String,
}

struct Bot;

#[allow(unused)]
#[derive(Deserialize)]
struct TablebaseResponse {
    wdl: i32,
    dtz: Option<i32>,
    dtm: Option<i32>,
    checkmate: bool,
    stalemate: bool,
    variant_win: Option<bool>,
    variant_loss: Option<bool>,
    insufficient_material: bool,
    moves: Vec<TablebaseResponseMove>,
}

#[allow(unused)]
#[derive(Deserialize)]
struct TablebaseResponseMove {
    uci: String,
    san: String,
    wdl: i32,
    dtz: Option<i32>,
    dtm: Option<i32>,
    zeroing: bool,
    checkmate: bool,
    stalemate: bool,
    variant_win: Option<bool>,
    variant_loss: Option<bool>,
    insufficient_material: bool,
}

impl Bot {
    fn new() -> Bot {
        Bot
    }

    fn setup_events(mut self) {
        let url = format!("{}{}", ENDPOINT_BASE, ENDPOINT_STREAM_EVENT);
        let mut easy = Easy::new();
        let easy = &mut easy;
        easy.url(&url).unwrap();
        let mut headers = List::new();
        headers.append(&format!("Authorization: Bearer {}", TOKEN)).unwrap();
        easy.http_headers(headers).unwrap();
        easy.write_function(move |mut data| {
            let mut s = "".to_string();
            data.read_to_string(&mut s).unwrap();
            let size = s.len();
            for line in s.split("\n") {
                self.on_event(line.to_string());
            }
            Ok(size)
        }).unwrap();
        easy.perform().unwrap();
    }

    fn on_event(&mut self, s: String) {
        if s.is_empty() || s == "\n" || s.trim().is_empty() || s.trim_end_matches("\n").is_empty() {
            return;
        }
        if let Ok(challenge) = serde_json::from_str::<ChallengeEvent>(&s) {
            let challenge = challenge.challenge;
            println!("Challenge from {}! Accepting", challenge.challenger.name);
            post::<Nothing>(ENDPOINT_ACCEPT, vec!(challenge.id), Option::None);
        } else if let Ok(game) = serde_json::from_str::<GameStartEvent>(&s) {
            let game = game.game;
            println!("Started game {}!", game.id);
            let game = Game::new(game.id);
            game.setup_events();
        } else {
            println!("Unknown event: {}", s)
        }
    }
}

#[derive(Serialize)]
struct Nothing;

#[allow(unused)]
#[derive(Deserialize)]
struct GameStartEvent {
    #[serde(rename = "type")]
    typ: String,
    game: GameStartEventGame,
}

#[derive(Deserialize)]
struct GameStartEventGame {
    id: String
}

#[allow(unused)]
#[derive(Deserialize)]
struct ChallengeEvent {
    #[serde(rename = "type")]
    typ: String,
    challenge: ChallengeEventChallenge,
}

#[allow(unused)]
#[derive(Deserialize)]
struct ChallengeEventChallenge {
    id: String,
    url: String,
    status: String,
    challenger: Player,
    #[serde(rename = "destUser")]
    dest_user: Player,
    variant: Variant,
    rated: bool,
    speed: String,
    #[serde(rename = "timeControl")]
    time_control: TimeControl,
    color: String,
    perf: Perf,
}

#[allow(unused)]
#[derive(Deserialize)]
struct Perf {
    icon: Option<String>,
    name: String,
}

#[allow(unused)]
#[derive(Deserialize)]
struct TimeControl {
    #[serde(rename = "type")]
    typ: String,
    limit: u64,
    increment: u64,
    show: String,
}

#[allow(unused)]
#[derive(Deserialize)]
struct Variant {
    key: String,
    name: String,
    short: String,
}

#[allow(unused)]
#[derive(Deserialize)]
struct Player {
    id: String,
    name: String,
    title: Option<String>,
    rating: u32,
    provisional: Option<bool>,
    online: Option<bool>,
    lag: Option<u32>,
}

#[allow(unused)]
fn get(endpoint: &str, args: Vec<String>) -> String {
    let mut endpoint = endpoint.to_string();
    for arg in args {
        endpoint = endpoint.replacen("{}", &*arg, 1);
    }
    let url = format!("{}{}", ENDPOINT_BASE, endpoint);
    let client = reqwest::blocking::Client::new();
    client
        .get(&url)
        .bearer_auth(TOKEN)
        .send()
        .unwrap()
        .text()
        .unwrap()
}

fn post<T: Serialize + Sized>(endpoint: &str, args: Vec<String>, form: Option<T>) -> String {
    let mut endpoint = endpoint.to_string();
    for arg in args {
        endpoint = endpoint.replacen("{}", &*arg, 1);
    }
    let client = reqwest::blocking::Client::new();
    let url = format!("{}{}", ENDPOINT_BASE, endpoint);
    let mut req = client
        .post(&url)
        .bearer_auth(TOKEN);

    if let Some(form) = form {
        req = req.form(&form);
    }
    req
        .send()
        .unwrap()
        .text()
        .unwrap()
}

#[derive(Serialize)]
struct ChatMessage {
    room: String,
    text: String,
}

impl ChatMessage {
    #[allow(unused)]
    fn new(message: String) -> ChatMessage {
        ChatMessage {
            room: "player".to_string(),
            text: message,
        }
    }
}

struct Game {
    moves: Vec<Move>,
    table: Table,
    id: String,
    my_color: PlayerColor,
}

impl Game {
    fn new(id: String) -> Game {
        Game {
            moves: Vec::new(),
            table: Table::default(),
            id,
            my_color: Unknown,
        }
    }
    #[allow(unused)]
    fn chat(&self, message: &str) {
        post(ENDPOINT_CHAT, vec!(self.id.clone()), Option::Some(ChatMessage::new(message.to_string())));
    }
    fn setup_events(mut self) {
        std::thread::Builder::new().name(self.id.clone()).spawn(move || {
            let url = format!("{}{}", ENDPOINT_BASE, ENDPOINT_STREAM.replace("{}", &self.id));
            let mut easy = Easy::new();
            let easy = &mut easy;
            easy.url(&url).unwrap();
            let mut headers = List::new();
            headers.append(&format!("Authorization: Bearer {}", TOKEN)).unwrap();
            easy.http_headers(headers).unwrap();
            easy.write_function(move |mut data| {
                let mut s = "".to_string();
                data.read_to_string(&mut s).unwrap();
                let size = s.len();
                for line in s.split("\n") {
                    self.on_event(line.to_string());
                }
                Ok(size)
            }).unwrap();
            easy.perform().unwrap();
        }).unwrap();
    }

    fn on_event(&mut self, s: String) {
        if s.is_empty() || s == "\n" || s.trim().is_empty() || s.trim_end_matches("\n").is_empty() {
            return;
        }
        if let Ok(game_full) = serde_json::from_str::<GameFullEvent>(&s) {
            let color = match &game_full.white.name[..] {
                "TAKETAKETAKETAKE" => "White",
                _ => "Black"
            };
            println!("Game {} started! We are playing {}", game_full.id, color);
            self.my_color = if color == "White" { White } else { Black };
            if self.my_color == White {
                self.make_move()
            }
        } else if let Ok(game_state) = serde_json::from_str::<GameStateEvent>(&s) {
            self.moves = game_state.moves.split(" ").map(|s| Move::from_str(s)).collect();
            self.table.sync(&self.moves);
            match game_state.status.as_str() {
                "started" => {
                    if self.moves.len() % 2 == 0 && self.my_color == White || self.moves.len() % 2 == 1 && self.my_color == Black {
                        self.make_move();
                    }
                }
                "mate" => {
                    self.end();
                }
                "draw" => {
                    self.end()
                }
                "resign" => {
                    self.end()
                }
                "stalemate" => {
                    self.end()
                }
                "outoftime" => {
                    self.end()
                }
                _ => {
                    println!("Unknown game status: {}", game_state.status);
                }
            }
        } else if let Ok(chat_line) = serde_json::from_str::<ChatLineEvent>(&s) {
            println!("Chat: {}: {}", chat_line.username, chat_line.text);
        } else {
            println!("Unknown event: {}", s)
        }
    }

    fn end(&mut self) {
        // TODO
    }

    fn resign(&mut self) {
        post::<Nothing>(ENDPOINT_RESIGN, vec!(self.id.clone()), Option::None);
    }

    fn make_move(&mut self) {
        let t = SystemTime::now();
        self.table.print();
        if self.table.pieces().len() <= 7 {
            // tablebase on
            println!("Tablebase on");
            let m = get_tablebase_move(&self.table, self.my_color);
            if m.is_some() && self.send_move(m.unwrap()) {
                self.table.process_move(m.unwrap());
                return;
            }
        } else if self.table.score(opposite(self.my_color)) - self.table.score(self.my_color) > 20 && self.table.score(self.my_color) < 20 {
            self.resign();
            return;
        }
        let mut moves: Vec<(i32, Move)> = self.table.available_moves(self.my_color).iter().filter(|x| self.table.is_move_legal(x)).map(|x| (self.table.move_score(*x), *x)).collect();
        moves.sort_by(|a, b| b.0.cmp(&a.0));
        let mut i = 0;

        loop {
            if thread_rng().gen_bool(0.15) && i + 1 < moves.len() {
                i += 1;
            }
            if i >= moves.len() {
                self.resign();
                return;
            }
            let (v, m) = moves[i];
            if self.table.assume_move(m).in_check(self.my_color) || !self.send_move(m) {
                i += 1;
                moves.remove(moves.iter().position(|x| x == &(v, m)).unwrap_or(0));
                if i < 100000 {
                    continue;
                }
            }
            self.table.process_move(m);
            break;
        }
        self.table.print();
        println!("Elapsed: {}s", t.elapsed().unwrap().as_secs_f64());
    }

    fn send_move(&self, m: Move) -> bool {
        let res = post::<Nothing>(ENDPOINT_MOVE, vec!(self.id.clone(), m.to_string()), Option::None);
        if let Ok(err) = serde_json::from_str::<ErrorResponse>(&res) {
            if err.error == "Not your turn, or game already over" {
                true
            } else {
                println!("{}", err.error);
                false
            }
        } else {
            true
        }
    }
}

#[allow(unused)]
#[derive(Deserialize)]
struct OkResponse {
    ok: bool
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: String
}

#[allow(unused)]
#[derive(Deserialize)]
struct ChatLineEvent {
    #[serde(rename = "type")]
    typ: String,
    room: String,
    username: String,
    text: String,
}

#[allow(unused)]
#[derive(Deserialize)]
struct GameFullEvent {
    id: String,
    variant: Variant,
    clock: Option<Clock>,
    speed: String,
    perf: Perf,
    rated: bool,
    #[serde(rename = "createdAt")]
    created_at: u64,
    white: Player,
    black: Player,
    #[serde(rename = "initialFen")]
    initial_fen: String,
    #[serde(rename = "type")]
    typ: String,
    state: GameStateEvent,
}

#[allow(unused)]
#[derive(Deserialize)]
struct GameStateEvent {
    #[serde(rename = "type")]
    typ: String,
    moves: String,
    wtime: i64,
    btime: i64,
    winc: i64,
    binc: i64,
    wdraw: bool,
    bdraw: bool,
    status: String,
}

#[allow(unused)]
#[derive(Deserialize)]
struct Clock {
    initial: usize,
    increment: usize,
}

#[derive(PartialEq, Clone)]
struct Table([[Piece; 8]; 8]);

impl Default for Table {
    fn default() -> Self {
        Table([
            [Piece::WhiteRook, Piece::WhiteKnight, Piece::WhiteBishop, Piece::WhiteKing, Piece::WhiteQueen, Piece::WhiteBishop, Piece::WhiteKnight, Piece::WhiteRook],
            [Piece::WhitePawn, Piece::WhitePawn, Piece::WhitePawn, Piece::WhitePawn, Piece::WhitePawn, Piece::WhitePawn, Piece::WhitePawn, Piece::WhitePawn],
            [Piece::None, Piece::None, Piece::None, Piece::None, Piece::None, Piece::None, Piece::None, Piece::None],
            [Piece::None, Piece::None, Piece::None, Piece::None, Piece::None, Piece::None, Piece::None, Piece::None],
            [Piece::None, Piece::None, Piece::None, Piece::None, Piece::None, Piece::None, Piece::None, Piece::None],
            [Piece::None, Piece::None, Piece::None, Piece::None, Piece::None, Piece::None, Piece::None, Piece::None],
            [Piece::BlackPawn, Piece::BlackPawn, Piece::BlackPawn, Piece::BlackPawn, Piece::BlackPawn, Piece::BlackPawn, Piece::BlackPawn, Piece::BlackPawn],
            [Piece::BlackRook, Piece::BlackKnight, Piece::BlackBishop, Piece::BlackKing, Piece::BlackQueen, Piece::BlackBishop, Piece::BlackKnight, Piece::BlackRook],
        ])
    }
}

fn color_to_str(color: PlayerColor) -> String {
    if color == White {
        "white"
    } else if color == Black {
        "black"
    } else {
        "err"
    }.to_string()
}

impl Table {
    fn fen(&self, player_to_move: PlayerColor) -> String {
        let mut s = "".to_string();

        // position
        let mut none = 0;
        let mut none_now = false;
        for row in self.0.iter().rev() {
            for piece in row.iter().rev() {
                if piece == &Piece::None {
                    none_now = true;
                    none += 1;
                } else {
                    if none_now {
                        s += &*none.to_string();
                        none = 0;
                        none_now = false;
                    }
                    s += &*match color_to_str(piece.color()).as_str() {
                        "white" => piece.to_string().to_uppercase(),
                        "black" => piece.to_string().to_lowercase(),
                        _ => "err".to_string()
                    };
                }
            }
            if none_now {
                s += &*none.to_string();
                none = 0;
                none_now = false;
            }
            s += "/";
        }
        s = s[0..s.len() - 1].to_string();

        // who moves next
        s += " ";
        s += if player_to_move == White { "w" } else { "b" };

        // castling (castling in endgame?)
        s += " -";

        // en passant (not now)
        s += " -";

        // useless for tablebase
        s += " 0 1";

        s
    }
    fn in_check(&self, color: PlayerColor) -> bool {
        let king = if color == White { Piece::WhiteKing } else { Piece::BlackKing };
        let pieces = self.pieces_colored(color);
        let d = (Position(0, 0), king);
        let (king_pos, _) = pieces.iter().find(|(_, p)| p == &king).unwrap_or(&d);
        let opposite = if color == White { Black } else { White };
        for (pos, piece) in self.pieces_colored(opposite) {
            let m = Move::new(pos, king_pos.clone());
            if piece.moves(pos).contains(&m) && self.is_move_legal(&m) {
                return true;
            }
        }
        false
    }
    fn score(&self, color: PlayerColor) -> i32 {
        self.pieces_colored(color).iter().map(|x| x.1.value()).sum()
    }
    fn print(&self) {
        let mut white = true;
        let mut str = "".to_string();
        self.0.iter().for_each(|row| {
            row.iter().for_each(|piece| {
                let mut s = piece.to_string();
                s = piece.color().color().bold().on(if white { Color::Fixed(253) } else { Color::Fixed(238) }).paint(format!(" {} ", s)).to_string();
                str += &s;
                white = !white;
            });
            white = !white;
            str += "\n";
        });
        println!("{}", str)
    }
    fn pieces(&self) -> Vec<(Position, Piece)> {
        let mut pieces = Vec::new();

        for x in 1..9 {
            for y in 1..9 {
                let pos = Position(x, y);
                let piece = self.get_piece_at(pos);
                if piece != Piece::None {
                    pieces.push((pos, piece));
                }
            }
        }
        pieces
    }
    fn sync(&mut self, moves: &Vec<Move>) {
        let mut table = Table::default();
        let mut syncing = false;
        if table.0 == self.0 {
            syncing = true
        }
        for m in moves {
            table.process_move(*m);
            if syncing {
                self.process_move(*m);
            }
            if table.0 == self.0 {
                syncing = true
            }
        }
    }
    fn pieces_colored(&self, color: PlayerColor) -> Vec<(Position, Piece)> {
        self.pieces().iter_mut().filter(|p| p.1.color() == color).map(|p| *p).collect()
    }
    fn available_moves(&self, color: PlayerColor) -> Vec<Move> {
        let mut moves = Vec::new();
        let a = self.pieces_colored(color);
        for (pos, piece) in a {
            let mut p = piece.moves(pos);
            moves.append(&mut p);
        }
        moves.iter().filter(|m| self.is_move_legal(*m)).map(|m| *m).collect()
    }
    fn is_move_legal(&self, m: &Move) -> bool {
        let pos = m.a;
        let piece = self.get_piece_at(pos);
        let color = piece.color();
        let mut legal = true;
        if !m.b.valid() || !m.a.valid() || self.get_piece_at(m.b).color() == color {
            legal = false
        }
        let dist_x = m.b.0 - m.a.0;
        let dist_y = m.b.1 - m.a.1;
        let dir_x = dist_x.signum();
        let dir_y = dist_y.signum();
        match piece {
            Piece::None => legal = false,
            Piece::WhiteKing | Piece::BlackKing => {
                if max(dist_x.abs(), dist_y.abs()) != 1 {
                    legal = false
                }
            }
            Piece::WhiteQueen | Piece::BlackQueen => {
                if dir_x == 0 || dir_y == 0 {
                    // Like a rook
                    for i in 1..dist_x.abs() {
                        let i = dir_x * i;
                        if self.get_piece_at(pos.add(i, 0)) != Piece::None {
                            legal = false;
                        }
                    }
                    for i in 1..dist_y.abs() {
                        let i = dir_y * i;
                        if self.get_piece_at(pos.add(0, i)) != Piece::None {
                            legal = false;
                        }
                    }
                } else {
                    // Like a bishop
                    for i in 1..dist_x.abs() {
                        let x = dir_x * i;
                        let y = dir_y * i;
                        if self.get_piece_at(pos.add(x, y)) != Piece::None {
                            legal = false;
                        }
                    }
                }
            }
            Piece::WhiteRook | Piece::BlackRook => {
                for i in 1..dist_x.abs() {
                    let i = dir_x * i;
                    if self.get_piece_at(pos.add(i, 0)) != Piece::None {
                        legal = false;
                    }
                }
                for i in 1..dist_y.abs() {
                    let i = dir_y * i;
                    if self.get_piece_at(pos.add(0, i)) != Piece::None {
                        legal = false;
                    }
                }
            }
            Piece::WhiteKnight | Piece::BlackKnight => {}
            Piece::WhiteBishop | Piece::BlackBishop => {
                for i in 1..dist_x.abs() {
                    let x = dir_x * i;
                    let y = dir_y * i;
                    if self.get_piece_at(pos.add(x, y)) != Piece::None {
                        legal = false;
                    }
                }
            }
            Piece::WhitePawn => {
                if dir_y == -1 {
                    legal = false
                }
                if dir_x != 0 && self.get_piece_at(pos.add(dir_x, dist_y)) == Piece::None {
                    legal = false
                }
                if dist_x == 0 && self.get_piece_at(pos.add(0, 1)) != Piece::None {
                    legal = false
                }
                if dist_y == 2 && pos.1 != 2 {
                    legal = false
                }
            }
            Piece::BlackPawn => {
                if dir_y == 1 {
                    legal = false
                }
                if dir_x != 0 && self.get_piece_at(pos.add(dir_x, dir_y)) == Piece::None {
                    legal = false
                }
                if dist_x == 0 && self.get_piece_at(pos.add(0, dir_y)) != Piece::None {
                    legal = false
                }
                if dist_y == -2 && pos.1 != 7 {
                    legal = false
                }
            }
        }
        legal
    }
    fn assume_move(&self, m: Move) -> Table {
        let mut table = self.clone();
        table.process_move(m);
        table
    }
    fn get_piece_at(&self, pos: Position) -> Piece {
        self.0[pos.1 as usize - 1][7 - (pos.0 as usize - 1)]
    }
    fn set_piece_at(&mut self, pos: Position, piece: Piece) {
        self.0[pos.1 as usize - 1][7 - (pos.0 as usize - 1)] = piece;
    }

    fn process_move(&mut self, m: Move) {
        let mut piece = self.get_piece_at(m.a);

        // Pawn promotion (for now, queen only)
        if piece.value() == 1 && (m.b.1 == 8 || m.b.1 == 1) {
            piece = if piece.color() == White { Piece::WhiteQueen } else { Piece::BlackQueen };
        }

        // Castling
        if piece == Piece::WhiteKing {
            if m.a == Position::from_str("e1") && m.b == Position::from_str("c1") {
                self.process_move(Move::from_str("a1d1"));
            }
            if m.a == Position::from_str("e1") && m.b == Position::from_str("g1") {
                self.process_move(Move::from_str("h1f1"));
            }
        }
        if piece == Piece::BlackKing {
            if m.a == Position::from_str("e8") && m.b == Position::from_str("c8") {
                self.process_move(Move::from_str("a8d8"));
            }
            if m.a == Position::from_str("e8") && m.b == Position::from_str("g8") {
                self.process_move(Move::from_str("h8f8"));
            }
        }

        self.set_piece_at(m.b, piece);
        self.set_piece_at(m.a, Piece::None);
    }
    fn move_score(&self, m: Move) -> i32 {
        let mut score = 0;
        let piece = self.get_piece_at(m.a);
        let color = piece.color();
        let opp_color = opposite(color);
        let t = self.assume_move(m);

        if t.in_check(color) {
            return -1;
        }

        // take
        if self.get_piece_at(m.b).color() == opp_color {
            score += self.get_piece_at(m.b).value() * 17;
        }

        // attack
        for m2 in t.available_moves(color) {
            if m2.a == m.b && self.get_piece_at(m2.b).color() == opp_color {
                let mut can_defend = false;
                let t = t.assume_move(m2);
                for m3 in t.available_moves(opp_color) {
                    if m3.b == m2.b {
                        can_defend = true;
                        break;
                    }
                }
                if !can_defend {
                    score += self.get_piece_at(m2.b).value() * 2;
                }
            }
        }

        // developing
        if piece.value() > Piece::BlackPawn.value() && piece.value() != Piece::WhiteKing.value() && Table::default().get_piece_at(m.a) == piece {
            score += 10;
        }

        // giveaway
        for m2 in t.available_moves(opp_color) {
            if t.get_piece_at(m2.b).color() == color {
                let t = t.assume_move(m2);
                let mut defended = false;
                for m3 in t.available_moves(color) {
                    if m3.b == m2.b && t.get_piece_at(m2.a).value() > piece.value() {
                        defended = true;
                    }
                }
                if !defended {
                    score -= piece.value() * 18;
                }
            }
        }

        // center
        if (m.b.0 == 4 || m.b.0 == 5) && (m.b.1 == 4 || m.b.1 == 5) {
            score += 12;
        }

        score
    }
}

fn opposite(color: PlayerColor) -> PlayerColor {
    if color == White { Black } else { White }
}

#[derive(Copy, Clone, Debug, PartialOrd, PartialEq)]
enum Piece {
    BlackKing,
    BlackQueen,
    BlackRook,
    BlackKnight,
    BlackBishop,
    BlackPawn,
    WhiteKing,
    WhiteQueen,
    WhiteRook,
    WhiteKnight,
    WhiteBishop,
    WhitePawn,
    None,
}

impl Display for Piece {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.write_str(match self {
            Piece::None => " ",
            Piece::WhiteKing | Piece::BlackKing => "K",
            Piece::WhiteQueen | Piece::BlackQueen => "Q",
            Piece::WhiteRook | Piece::BlackRook => "R",
            Piece::WhiteKnight | Piece::BlackKnight => "N",
            Piece::WhiteBishop | Piece::BlackBishop => "B",
            Piece::WhitePawn | Piece::BlackPawn => "p",
        })
    }
}

impl Piece {
    fn value(&self) -> i32 {
        match self {
            Piece::None => 0,
            Piece::WhiteKing | Piece::BlackKing => 18,
            Piece::WhiteQueen | Piece::BlackQueen => 9,
            Piece::WhiteRook | Piece::BlackRook => 5,
            Piece::WhiteKnight | Piece::BlackKnight => 3,
            Piece::WhiteBishop | Piece::BlackBishop => 3,
            Piece::WhitePawn | Piece::BlackPawn => 1,
        }
    }
    fn color(&self) -> PlayerColor {
        match self {
            Piece::None => Unknown,
            Piece::BlackKing |
            Piece::BlackQueen |
            Piece::BlackRook |
            Piece::BlackKnight |
            Piece::BlackBishop |
            Piece::BlackPawn => Black,
            Piece::WhiteKing |
            Piece::WhiteQueen |
            Piece::WhiteRook |
            Piece::WhiteKnight |
            Piece::WhiteBishop |
            Piece::WhitePawn => White
        }
    }
    fn moves(&self, pos: Position) -> Vec<Move> {
        let mut moves = Vec::new();
        match self {
            Piece::None => (),
            Piece::WhiteKing | Piece::BlackKing => {
                moves.push(Move::new(pos, pos.add(1, 0)));
                moves.push(Move::new(pos, pos.add(1, 1)));
                moves.push(Move::new(pos, pos.add(0, 1)));
                moves.push(Move::new(pos, pos.add(-1, 0)));
                moves.push(Move::new(pos, pos.add(-1, -1)));
                moves.push(Move::new(pos, pos.add(0, -1)));
                moves.push(Move::new(pos, pos.add(-1, 1)));
                moves.push(Move::new(pos, pos.add(1, -1)));
            }
            Piece::WhiteQueen | Piece::BlackQueen => {
                moves.append(&mut Piece::WhiteBishop.moves(pos));
                moves.append(&mut Piece::WhiteRook.moves(pos));
            }
            Piece::WhiteRook | Piece::BlackRook => {
                moves.push(Move::new(pos, pos.add(0, 1)));
                moves.push(Move::new(pos, pos.add(0, 2)));
                moves.push(Move::new(pos, pos.add(0, 3)));
                moves.push(Move::new(pos, pos.add(0, 4)));
                moves.push(Move::new(pos, pos.add(0, 5)));
                moves.push(Move::new(pos, pos.add(0, 6)));
                moves.push(Move::new(pos, pos.add(0, 7)));
                moves.push(Move::new(pos, pos.add(0, -1)));
                moves.push(Move::new(pos, pos.add(0, -2)));
                moves.push(Move::new(pos, pos.add(0, -3)));
                moves.push(Move::new(pos, pos.add(0, -4)));
                moves.push(Move::new(pos, pos.add(0, -5)));
                moves.push(Move::new(pos, pos.add(0, -6)));
                moves.push(Move::new(pos, pos.add(0, -7)));
                moves.push(Move::new(pos, pos.add(1, 0)));
                moves.push(Move::new(pos, pos.add(2, 0)));
                moves.push(Move::new(pos, pos.add(3, 0)));
                moves.push(Move::new(pos, pos.add(4, 0)));
                moves.push(Move::new(pos, pos.add(5, 0)));
                moves.push(Move::new(pos, pos.add(6, 0)));
                moves.push(Move::new(pos, pos.add(7, 0)));
                moves.push(Move::new(pos, pos.add(-1, 0)));
                moves.push(Move::new(pos, pos.add(-2, 0)));
                moves.push(Move::new(pos, pos.add(-3, 0)));
                moves.push(Move::new(pos, pos.add(-4, 0)));
                moves.push(Move::new(pos, pos.add(-5, 0)));
                moves.push(Move::new(pos, pos.add(-6, 0)));
                moves.push(Move::new(pos, pos.add(-7, 0)));
            }
            Piece::WhiteKnight | Piece::BlackKnight => {
                moves.push(Move::new(pos, pos.add(1, 2)));
                moves.push(Move::new(pos, pos.add(2, 1)));
                moves.push(Move::new(pos, pos.add(1, -2)));
                moves.push(Move::new(pos, pos.add(2, -1)));
                moves.push(Move::new(pos, pos.add(-1, 2)));
                moves.push(Move::new(pos, pos.add(-2, 1)));
                moves.push(Move::new(pos, pos.add(-1, -2)));
                moves.push(Move::new(pos, pos.add(-2, -1)));
            }
            Piece::WhiteBishop | Piece::BlackBishop => {
                moves.push(Move::new(pos, pos.add(1, 1)));
                moves.push(Move::new(pos, pos.add(2, 2)));
                moves.push(Move::new(pos, pos.add(3, 3)));
                moves.push(Move::new(pos, pos.add(4, 4)));
                moves.push(Move::new(pos, pos.add(5, 5)));
                moves.push(Move::new(pos, pos.add(6, 6)));
                moves.push(Move::new(pos, pos.add(7, 7)));
                moves.push(Move::new(pos, pos.add(1, -1)));
                moves.push(Move::new(pos, pos.add(2, -2)));
                moves.push(Move::new(pos, pos.add(3, -3)));
                moves.push(Move::new(pos, pos.add(4, -4)));
                moves.push(Move::new(pos, pos.add(5, -5)));
                moves.push(Move::new(pos, pos.add(6, -6)));
                moves.push(Move::new(pos, pos.add(7, -7)));
                moves.push(Move::new(pos, pos.add(-1, 1)));
                moves.push(Move::new(pos, pos.add(-2, 2)));
                moves.push(Move::new(pos, pos.add(-3, 3)));
                moves.push(Move::new(pos, pos.add(-4, 4)));
                moves.push(Move::new(pos, pos.add(-5, 5)));
                moves.push(Move::new(pos, pos.add(-6, 6)));
                moves.push(Move::new(pos, pos.add(-7, 7)));
                moves.push(Move::new(pos, pos.add(-1, -1)));
                moves.push(Move::new(pos, pos.add(-2, -2)));
                moves.push(Move::new(pos, pos.add(-3, -3)));
                moves.push(Move::new(pos, pos.add(-4, -4)));
                moves.push(Move::new(pos, pos.add(-5, -5)));
                moves.push(Move::new(pos, pos.add(-6, -6)));
                moves.push(Move::new(pos, pos.add(-7, -7)));
            }
            Piece::WhitePawn => {
                moves.push(Move::new(pos, pos.add(0, 1)));
                moves.push(Move::new(pos, pos.add(0, 2)));
                moves.push(Move::new(pos, pos.add(1, 1)));
                moves.push(Move::new(pos, pos.add(-1, 1)));
            }
            Piece::BlackPawn => {
                moves.push(Move::new(pos, pos.add(0, -1)));
                moves.push(Move::new(pos, pos.add(0, -2)));
                moves.push(Move::new(pos, pos.add(1, -1)));
                moves.push(Move::new(pos, pos.add(-1, -1)));
            }
        };
        moves.iter().filter(|m| m.a.valid() && m.b.valid()).map(|m| *m).collect()
    }
}

#[derive(Copy, Clone, PartialEq)]
struct Move {
    a: Position,
    b: Position,
}

impl Move {
    pub fn new(a: Position, b: Position) -> Move { Move { a, b } }
    pub fn from_str(s: &str) -> Move {
        Move::new(Position::from_str(&s[0..2]), Position::from_str(&s[2..4]))
    }
}

impl Debug for Move {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.write_str(&*(self.a.to_string() + &*self.b.to_string()))
    }
}

impl Display for Move {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.write_str(&*(self.a.to_string() + &*self.b.to_string()))
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct Position(i64, i64);

impl Position {
    fn valid(&self) -> bool { self.0 >= 1 && self.0 <= 8 && self.1 >= 1 && self.1 <= 8 }
    fn add(&self, x: i64, y: i64) -> Position { Position(self.0 + x, self.1 + y) }
    fn from_str(s: &str) -> Position {
        let ch1 = &s[0..1];
        let ch2 = &s[1..2];
        let x = A1.iter().position(|x| x == &ch1).unwrap();
        let y = A2.iter().position(|x| x == &ch2).unwrap();
        Position(x as i64 + 1, y as i64 + 1)
    }
}

impl Display for Position {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.write_str(&*(A1[self.0 as usize - 1].to_string() + &A2[self.1 as usize - 1].to_string()))
    }
}