use crate::hand::Counts;
use crate::tile::Tile;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum SetKind {
    Chow,
    Pung,
    Kong,
}

/// A completed set. For a chow, `tile` is the lowest tile.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Set {
    pub kind: SetKind,
    pub tile: Tile,
    /// True if formed by calling another player's discard.
    pub open: bool,
}

impl Set {
    pub fn chow(tile: Tile, open: bool) -> Set {
        Set {
            kind: SetKind::Chow,
            tile,
            open,
        }
    }

    pub fn pung(tile: Tile, open: bool) -> Set {
        Set {
            kind: SetKind::Pung,
            tile,
            open,
        }
    }

    pub fn kong(tile: Tile, open: bool) -> Set {
        Set {
            kind: SetKind::Kong,
            tile,
            open,
        }
    }
}

/// One way of reading a complete hand as four sets and a pair.
/// `sets` lists declared melds first, then the sets found in the concealed tiles.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Decomposition {
    pub pair: Tile,
    pub sets: Vec<Set>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum WinShape {
    Standard(Decomposition),
    /// Seven distinct pairs (four of a kind does not count as two pairs).
    SevenPairs,
    ThirteenOrphans,
}

/// All ways the hand forms a complete winning shape.
///
/// `concealed` holds the tiles in hand (including the winning tile) and `melds`
/// the declared sets. An empty result means the hand is not a win.
pub fn win_shapes(concealed: &Counts, melds: &[Set]) -> Vec<WinShape> {
    let mut shapes = Vec::new();
    if melds.len() > 4 {
        return shapes;
    }
    let needed = 4 - melds.len();
    if concealed.total() != needed * 3 + 2 {
        return shapes;
    }

    let mut work = concealed.clone();
    for pair in Tile::all() {
        if work.get(pair) < 2 {
            continue;
        }
        work.remove_n(pair, 2);
        for found in split_sets(&mut work, needed) {
            let mut sets = melds.to_vec();
            sets.extend(found);
            shapes.push(WinShape::Standard(Decomposition { pair, sets }));
        }
        work.add_n(pair, 2);
    }

    if melds.is_empty() {
        if is_seven_pairs(concealed) {
            shapes.push(WinShape::SevenPairs);
        }
        if is_thirteen_orphans(concealed) {
            shapes.push(WinShape::ThirteenOrphans);
        }
    }
    shapes
}

/// Tiles that would complete a hand holding one tile fewer than a winning hand.
pub fn winning_tiles(concealed: &Counts, melds: &[Set]) -> Vec<Tile> {
    if melds.len() > 4 || concealed.total() != (4 - melds.len()) * 3 + 1 {
        return Vec::new();
    }
    let mut work = concealed.clone();
    let mut out = Vec::new();
    for tile in Tile::all() {
        if work.get(tile) >= 4 {
            continue;
        }
        work.add(tile);
        if !win_shapes(&work, melds).is_empty() {
            out.push(tile);
        }
        work.remove_n(tile, 1);
    }
    out
}

/// Every way to split exactly `needed` sets out of `counts` using all of its tiles.
/// Always consumes the lowest remaining tile, so each result is unique.
fn split_sets(counts: &mut Counts, needed: usize) -> Vec<Vec<Set>> {
    if needed == 0 {
        return if counts.total() == 0 {
            vec![Vec::new()]
        } else {
            Vec::new()
        };
    }
    let Some(first) = Tile::all().find(|&t| counts.get(t) > 0) else {
        return Vec::new();
    };

    let mut results = Vec::new();

    if counts.get(first) >= 3 {
        counts.remove_n(first, 3);
        for mut rest in split_sets(counts, needed - 1) {
            rest.insert(0, Set::pung(first, false));
            results.push(rest);
        }
        counts.add_n(first, 3);
    }

    if first.can_start_chow() {
        let (second, third) = (first.offset(1), first.offset(2));
        if counts.get(second) > 0 && counts.get(third) > 0 {
            counts.remove_n(first, 1);
            counts.remove_n(second, 1);
            counts.remove_n(third, 1);
            for mut rest in split_sets(counts, needed - 1) {
                rest.insert(0, Set::chow(first, false));
                results.push(rest);
            }
            counts.add_n(first, 1);
            counts.add_n(second, 1);
            counts.add_n(third, 1);
        }
    }

    results
}

fn is_seven_pairs(counts: &Counts) -> bool {
    counts.total() == 14 && Tile::all().all(|t| matches!(counts.get(t), 0 | 2))
}

fn is_thirteen_orphans(counts: &Counts) -> bool {
    if counts.total() != 14 {
        return false;
    }
    let mut orphan_tiles = 0;
    let mut orphan_total = 0;
    for tile in Tile::all().filter(|t| t.is_terminal_or_honor()) {
        let c = counts.get(tile) as usize;
        if c == 0 {
            return false;
        }
        orphan_tiles += 1;
        orphan_total += c;
    }
    orphan_tiles == 13 && orphan_total == 14
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shapes(s: &str) -> Vec<WinShape> {
        win_shapes(&Counts::parse(s).unwrap(), &[])
    }

    fn standard_count(shapes: &[WinShape]) -> usize {
        shapes
            .iter()
            .filter(|s| matches!(s, WinShape::Standard(_)))
            .count()
    }

    #[test]
    fn simple_win() {
        let found = shapes("123m456p789s11122z");
        assert_eq!(standard_count(&found), 1);
        let WinShape::Standard(d) = &found[0] else {
            panic!("expected standard shape");
        };
        assert_eq!(d.pair, Tile(28));
        assert_eq!(d.sets.len(), 4);
    }

    #[test]
    fn not_a_win() {
        assert!(shapes("123456789m12345p").is_empty());
        assert!(shapes("124m456p789s11122z").is_empty());
        // wrong tile count
        assert!(shapes("123m456p789s1122z").is_empty());
    }

    #[test]
    fn ambiguous_hand_has_two_readings() {
        // 111222333m can be three pungs or three identical chows.
        let found = shapes("111222333m456p11z");
        assert_eq!(standard_count(&found), 2);
    }

    #[test]
    fn declared_melds_count_as_sets() {
        let melds = [
            Set::pung(Tile(31), true), // pung of white dragon
            Set::kong(Tile(27), false),
        ];
        let concealed = Counts::parse("123m456p99s").unwrap();
        let found = win_shapes(&concealed, &melds);
        assert_eq!(standard_count(&found), 1);
        let WinShape::Standard(d) = &found[0] else {
            panic!()
        };
        assert_eq!(d.sets.len(), 4);
        assert_eq!(d.sets[0], melds[0]);
    }

    #[test]
    fn seven_pairs() {
        let found = shapes("1133m5588p2277s44z");
        assert!(found.contains(&WinShape::SevenPairs));
        // four of a kind is not two pairs
        assert!(!shapes("1111m3355p2277s44z").contains(&WinShape::SevenPairs));
    }

    #[test]
    fn thirteen_orphans() {
        assert!(shapes("19m19p19s12345677z").contains(&WinShape::ThirteenOrphans));
        // missing the 7z orphan
        assert!(!shapes("19m19p19s12345666z").contains(&WinShape::ThirteenOrphans));
    }

    #[test]
    fn waits() {
        let nine_gates = Counts::parse("1112345678999m").unwrap();
        let waits = winning_tiles(&nine_gates, &[]);
        let expected: Vec<Tile> = (0..9).map(Tile).collect();
        assert_eq!(waits, expected);

        let two_sided = Counts::parse("23m456p789s11122z").unwrap();
        assert_eq!(winning_tiles(&two_sided, &[]), vec![Tile(0), Tile(3)]);

        let orphans = Counts::parse("19m19p19s1234567z").unwrap();
        assert_eq!(winning_tiles(&orphans, &[]).len(), 13);
    }
}
