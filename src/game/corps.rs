//! 軍団の編成・選択と操作パネル。

use super::*;

pub(super) const CORPS_SLOTS: usize = 10;

impl Game {
    pub(super) fn corps_controls_available(&self) -> bool {
        self.screen == Screen::Playing
            && !(self.show_diplo
                || self.show_help
                || self.show_chron
                || self.show_market
                || self.show_summon
                || self.show_save)
            && !self.peace_pending()
            && self.world.game_over.is_none()
    }

    pub(super) fn selected_soldiers(&self) -> Vec<Handle> {
        self.sel
            .iter()
            .copied()
            .filter(|&h| self.is_corps_member(h))
            .collect()
    }

    fn is_corps_member(&self, h: Handle) -> bool {
        self.world
            .unit(h)
            .is_some_and(|u| u.nation as usize == self.world.player && u.kind.military())
    }

    pub(super) fn prune_corps(&mut self) {
        let world = &self.world;
        let player = world.player;
        for corps in &mut self.corps {
            corps.retain(|&h| {
                world
                    .unit(h)
                    .is_some_and(|u| u.nation as usize == player && u.kind.military())
            });
        }
        if let Some(slot) = self.active_corps {
            if self.corps[slot].is_empty() {
                self.clear_sel();
            } else if self.sel != self.corps[slot] {
                let soldiers = self.corps[slot].clone();
                let show_corps = self.show_corps;
                self.set_sel(soldiers);
                self.active_corps = Some(slot);
                self.show_corps = show_corps;
            }
        }
    }

    pub(super) fn reset_corps(&mut self) {
        self.corps.iter_mut().for_each(Vec::clear);
        self.active_corps = None;
        self.show_corps = false;
    }

    pub(super) fn form_corps(&mut self) {
        self.prune_corps();
        let soldiers = self.selected_soldiers();
        if soldiers.len() < 2 {
            self.notice(t!(
                "自軍の軍事ユニットを2体以上選択してください",
                "Select at least two of your military units",
                "请至少选择两个己方军事单位"
            ));
            return;
        }
        if self.active_corps.is_some() {
            return;
        }
        let Some(slot) = self.corps.iter().position(Vec::is_empty) else {
            self.notice(t!(
                "軍団が上限に達しました。既存の軍団を解散してください",
                "Corps limit reached. Disband a corps to free a slot.",
                "军团数量已达上限。请先解散一个军团。"
            ));
            return;
        };
        for corps in &mut self.corps {
            corps.retain(|h| !soldiers.contains(h));
        }
        self.corps[slot] = soldiers;
        self.select_corps(slot);
        self.notice(&tf!(
            "第{}軍団を編成しました",
            "Formed Corps {}",
            "已组建第{}军团",
            slot + 1
        ));
    }

    pub(super) fn select_corps(&mut self, slot: usize) {
        let soldiers: Vec<_> = self.corps[slot]
            .iter()
            .copied()
            .filter(|&h| self.is_corps_member(h))
            .collect();
        if soldiers.is_empty() {
            return;
        }
        self.mode = Mode::Normal;
        self.ldown = None;
        let show_corps = self.show_corps;
        self.set_sel(soldiers);
        self.active_corps = Some(slot);
        self.show_corps = show_corps;
    }

    pub(super) fn disband_corps(&mut self) {
        if let Some(slot) = self.active_corps.take() {
            self.corps[slot].clear();
            self.mode = Mode::Normal;
            self.show_corps = false;
            self.notice(&tf!(
                "第{}軍団を解散しました",
                "Disbanded Corps {}",
                "已解散第{}军团",
                slot + 1
            ));
        }
    }

    pub(super) fn render_corps(&mut self, x: f32, y: f32, width: f32) {
        let title = if let Some(slot) = self.active_corps {
            tf!(
                "第{}軍団　{}体",
                "Corps {} · {} units",
                "第{}军团　{}个单位",
                slot + 1,
                self.corps[slot].len()
            )
        } else {
            t!("軍団", "Corps", "军团").to_string()
        };
        self.gfx.text_shadow(
            &title,
            x,
            y,
            width - 94.0,
            28.0,
            Font::Big,
            [0.95, 0.92, 0.85, 1.0],
        );
        let gap = 4.0;
        let button_w = (width - 4.0 * gap) / 5.0;
        for slot in 0..CORPS_SLOTS {
            let x0 = x + (slot % 5) as f32 * (button_w + gap);
            let y0 = y + 30.0 + (slot / 5) as f32 * 38.0;
            let count = self.corps[slot].len();
            let label = if self.active_corps == Some(slot) {
                format!("▶ {}", slot + 1)
            } else {
                (slot + 1).to_string()
            };
            let count = count.to_string();
            self.add_button(
                x0,
                y0,
                x0 + button_w,
                y0 + 34.0,
                &label,
                &count,
                0,
                !self.corps[slot].is_empty(),
                Act::SelectCorps(slot),
                t!(
                    "この軍団を呼び出し、進軍や防衛を命じる",
                    "Recall this corps to issue advance or defense orders",
                    "召回该军团并下达进军或防卫命令"
                ),
            );
        }
        let y0 = y + 112.0;
        let soldiers = self.selected_soldiers().len();
        let half = (width - gap) / 2.0;
        self.add_button(x, y0, x + half, y0 + 36.0, t!("軍団を編成", "Form corps", "组建军团"), "", 0,
            soldiers >= 2 && self.active_corps.is_none(), Act::FormCorps,
            t!("選択中の自軍の軍事ユニットで軍団を編成。所属済みの兵は新しい軍団に移る。", "Form a corps from your selected military units. Troops assigned elsewhere transfer to the new corps.", "用所选己方军事单位组建军团。已有归属的单位转入新军团。"));
        self.add_button(
            x + half + gap,
            y0,
            x + width,
            y0 + 36.0,
            t!("軍団を解散", "Disband corps", "解散军团"),
            "",
            0,
            self.active_corps.is_some(),
            Act::DisbandCorps,
            t!(
                "呼び出し中の軍団を解散する。各ユニットは現在の命令を続ける。",
                "Disband the current corps. Its units keep their orders.",
                "解散当前军团。各单位继续执行原有命令。"
            ),
        );
    }
}
