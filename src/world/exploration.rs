use super::*;

const ORDERS_PER_UPDATE: usize = 8;
const SEARCH_BUDGET: usize = 30_000;

/// 探索先の視界で新たに見えるマスを数える。ほかの探索隊の目的地周辺は予約する。
struct Coverage {
    fog: Vec<u8>,
    row_totals: Vec<u16>,
}

impl Coverage {
    fn new(fog: &[u8]) -> Self {
        let mut coverage = Self { fog: fog.to_vec(), row_totals: vec![0; (MAP_W + 1) * MAP_H] };
        for y in 0..MAP_H {
            coverage.update_row(y);
        }
        coverage
    }

    fn update_row(&mut self, y: usize) {
        let row = y * (MAP_W + 1);
        for (x, &f) in self.fog[y * MAP_W..(y + 1) * MAP_W].iter().enumerate() {
            self.row_totals[row + x + 1] = self.row_totals[row + x] + u16::from(f == 0);
        }
    }

    fn gain(&self, x: i32, y: i32, spans: &[i32]) -> u32 {
        let radius = (spans.len() / 2) as i32;
        let mut total = 0;
        for (dy, &span) in (-radius..=radius).zip(spans) {
            let ny = y + dy;
            if (0..MAP_H as i32).contains(&ny) {
                let row = ny as usize * (MAP_W + 1);
                let left = (x - span).max(0) as usize;
                let right = (x + span + 1).min(MAP_W as i32) as usize;
                total += u32::from(self.row_totals[row + right] - self.row_totals[row + left]);
            }
        }
        total
    }

    fn reserve(&mut self, goal: V2, spans: &[i32]) {
        let (x, y) = goal.tile();
        if !Map::inb(x, y) {
            return;
        }
        let radius = (spans.len() / 2) as i32;
        for (dy, &span) in (-radius..=radius).zip(spans) {
            let ny = y + dy;
            if (0..MAP_H as i32).contains(&ny) {
                let row = ny as usize * MAP_W;
                let left = (x - span).max(0) as usize;
                let right = (x + span + 1).min(MAP_W as i32) as usize;
                self.fog[row + left..row + right].fill(1);
                self.update_row(ny as usize);
            }
        }
    }
}

fn sight_spans(radius: i32) -> Vec<i32> {
    (-radius..=radius)
        .map(|dy| (0..=radius).rev().find(|dx| dx * dx + dy * dy <= radius * radius + radius).unwrap_or(0))
        .collect()
}

fn has_unexplored(fog: &[u8], x: i32, y: i32, spans: &[i32]) -> bool {
    let radius = (spans.len() / 2) as i32;
    (-radius..=radius).zip(spans).any(|(dy, &span)| {
        let ny = y + dy;
        if !(0..MAP_H as i32).contains(&ny) {
            return false;
        }
        let row = ny as usize * MAP_W;
        let left = (x - span).max(0) as usize;
        let right = (x + span + 1).min(MAP_W as i32) as usize;
        fog[row + left..row + right].contains(&0)
    })
}

/// 到達できる地点を近い順に調べ、最初の候補の近くで視界が広く開ける地点を選ぶ。
fn find_goal(cost: &[f32], start: (i32, i32), spans: &[i32], coverage: &Coverage, parents: &mut [usize]) -> (Vec<(i16, i16)>, usize) {
    if !Map::inb(start.0, start.1) {
        return (Vec::new(), 0);
    }
    let start = Map::idx(start.0, start.1);
    parents.fill(usize::MAX);
    parents[start] = start;
    let mut queue = VecDeque::from([(start, 0)]);
    let mut best = None;
    let mut best_score = 0;
    let mut last_distance = usize::MAX;
    let mut searched = 0;
    while let Some((tile, distance)) = queue.pop_front() {
        if distance > last_distance {
            break;
        }
        searched += 1;
        let (x, y) = ((tile % MAP_W) as i32, (tile / MAP_W) as i32);
        if distance > 0 {
            let gain = coverage.gain(x, y, spans) as usize;
            if gain > 0 {
                last_distance = last_distance.min(distance + spans.len() / 2);
                let score = gain * 1024 / (distance + spans.len());
                if best.is_none() || score > best_score {
                    best = Some(tile);
                    best_score = score;
                }
            }
        }
        if distance < last_distance {
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if Map::inb(nx, ny) {
                    let next = Map::idx(nx, ny);
                    if cost[next].is_finite() && parents[next] == usize::MAX {
                        parents[next] = tile;
                        queue.push_back((next, distance + 1));
                    }
                }
            }
        }
    }
    let mut path = Vec::new();
    if let Some(mut tile) = best {
        while tile != start {
            path.push(((tile % MAP_W) as i16, (tile / MAP_W) as i16));
            tile = parents[tile];
        }
        path.reverse();
    }
    (path, searched)
}

impl World {
    pub fn order_explore(&mut self, handles: &[Handle]) {
        self.note_player_order(handles);
        let player = self.player;
        for &handle in handles {
            if let Some(unit) = self.unit_mut(handle).filter(|u| u.alive && u.nation as usize == player) {
                unit.order = Order::Explore(None);
                unit.resume = None;
                unit.build_queue.clear();
                unit.reset_nav();
                unit.path_pending = false;
                unit.scan_t = 0.0;
            }
        }
    }

    /// 霧の更新後に次の目的地を配分する。探索命令は部隊の選択を解いても続く。
    pub fn plan_exploration(&mut self, fog: &[u8]) {
        let mut scouts = self.units.iter_mut().enumerate().filter(|(_, u)| {
            u.alive && u.nation as usize == self.player && matches!(u.order, Order::Explore(_))
        }).peekable();
        if scouts.peek().is_none() {
            return;
        }

        let mut pending = Vec::new();
        let mut assigned = Vec::new();
        for (i, u) in scouts {
            if let Order::Explore(Some(goal)) = u.order {
                let (x, y) = goal.tile();
                let spans = sight_spans(u.kind.vision());
                if has_unexplored(fog, x, y, &spans) {
                    assigned.push((goal, spans));
                    continue;
                }
                u.order = Order::Explore(None);
                u.reset_nav();
                u.path_pending = false;
            }
            if u.scan_t <= 0.0 {
                pending.push(i);
            }
        }
        if pending.is_empty() {
            return;
        }

        let mut coverage = Coverage::new(fog);
        let mut has_assignments = !assigned.is_empty();
        for (goal, spans) in assigned {
            coverage.reserve(goal, &spans);
        }
        let mut parents = vec![usize::MAX; MAP_W * MAP_H];
        let mut searched = 0;
        for i in pending.into_iter().take(ORDERS_PER_UPDATE) {
            let u = &mut self.units[i];
            let spans = sight_spans(u.kind.vision());
            let cost = &self.cost;
            let (path, visited) = find_goal(cost, u.pos.tile(), &spans, &coverage, &mut parents);
            searched += visited;
            if let Some(&(x, y)) = path.last() {
                let goal = v2(x as f32 + 0.5, y as f32 + 0.5);
                u.reset_nav();
                u.path = path;
                u.path_goal = (x as i32, y as i32);
                u.path_pending = false;
                u.order = Order::Explore(Some(goal));
                coverage.reserve(goal, &spans);
                has_assignments = true;
            } else if has_assignments {
                // ほかの隊が予約した範囲しか残っていない場合は、到着や命令変更を待つ。
                u.scan_t = 2.0;
            } else {
                u.order = Order::Idle;
            }
            if searched >= SEARCH_BUDGET {
                break;
            }
        }
    }

    pub(super) fn update_explorer(&mut self, i: usize, goal: V2, dt: f32) {
        // 建設などで通れなくなった経路は、次の霧更新時に引き直す。
        let u = &self.units[i];
        let blocked = u.path.get(u.path_i).is_some_and(|&(x, y)| {
            let p = v2(x as f32 + 0.5, y as f32 + 0.5);
            !self.passable_at(p)
        });
        if blocked || self.nav(i, goal, 0.25, dt) {
            let u = &mut self.units[i];
            u.order = Order::Explore(None);
            u.reset_nav();
            u.path_pending = false;
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scouts_share_reachable_frontiers() {
        let mut fog = vec![1; MAP_W * MAP_H];
        // 左右に未踏地があり、その外にも通行不能な未踏地がある。
        for y in 9..=11 {
            for x in (11..=12).chain(28..=29) {
                fog[Map::idx(x, y)] = 0;
            }
        }
        fog[Map::idx(20, 25)] = 0;
        let mut cost = vec![f32::INFINITY; fog.len()];
        for x in 10..=30 {
            cost[Map::idx(x, 10)] = 1.0;
        }
        let mut coverage = Coverage::new(&fog);
        let spans = sight_spans(6);
        let mut parents = vec![usize::MAX; cost.len()];
        let mut goals = Vec::new();
        for _ in 0..2 {
            let (path, _) = find_goal(&cost, (20, 10), &spans, &coverage, &mut parents);
            let &(x, y) = path.last().expect("a reachable frontier");
            assert!(path.iter().all(|&(x, y)| cost[Map::idx(x as i32, y as i32)].is_finite()));
            coverage.reserve(v2(x as f32 + 0.5, y as f32 + 0.5), &spans);
            goals.push(x);
        }
        assert_ne!(goals[0] < 20, goals[1] < 20);
        let (path, _) = find_goal(&cost, (20, 10), &spans, &coverage, &mut parents);
        assert!(path.is_empty());
    }
}
