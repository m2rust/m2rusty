# C++ / Python behaviour not yet ported

What the original client (`client/src/ui`, `client/src/game`, `bin/pack/root/*.py`)
does that the Rust port does not do yet, or does only partly. Gathered on
2026-11-02 by reading the C++ handlers and the Python UI against
`crates/m2-game` and `crates/m2-ui`. Items marked **state only** have their
packets handled (`m2_game::social::SocialState`, `PlayerState`) but no window.

## Network packets

Every game-phase `HEADER_GC_*` the C++ dispatches now has a Rust handler. The
ones below keep state or raise a `SocialEvent`, but nothing draws or answers it:

| Packet | C++ / Python | Rust today |
|---|---|---|
| `GC_EXCHANGE` | `CPythonExchange`, `uiexchange.py` (trade window, accept, Yang) | state only (`SocialState::exchange`); no window; no `CG_EXCHANGE` requests |
| `GC_GUILD` (all 20 subheaders) | `CPythonGuild`, `uiguild.py` (info/members/grades/board/skills/war pages) | state only; GVG war keys feed combat; no guild window, no guild marks |
| `GC_MESSENGER` | `CPythonMessenger`, `uimessenger.py` (friend list, guild online list) | state only; no window, no add/remove friend |
| `GC_PARTY_INVITE`, `GC_PARTY_PARAMETER` | `RecvPartyInviteQuestion`, `uiparty.py` (member gauges, distribution mode) | `SocialEvent::PartyInvite` raised; no question dialog, no party window |
| `GC_QUEST_CONFIRM`, `GC_REQUEST_MAKE_GUILD`, guild invite | `BINARY_OnQuestConfirm`, `AskGuildName`, `RecvGuildInviteQuestion` | events raised; no dialogs, no answers sent |
| `GC_REFINE_INFORMATION(_NEW)` | `uirefine.py` (upgrade dialog with materials, cost, chance) | state only (`SocialState::refine`); no dialog, no `CG_REFINE` |
| `GC_DRAGON_SOUL_REFINE` | `uidragonsoul.py` | event only |
| `GC_VIEW_EQUIP` | `uiequipmentdialog.py` (inspect another player) | state only |
| `GC_SHOP_SIGN` | `BINARY_PrivateShop_Appear` (sign board over the seller) | state only; signs not drawn; no private shop builder (`uiprivateshopbuilder.py`) |
| `GC_NPC_POSITION`, `GC_LAND_LIST`, `GC_TARGET_*` | atlas marks, guild areas, quest target arrows (`CPythonMiniMap`, `CreateTargetEffect`) | state only; the mini map/atlas does not draw them; no ground arrow effect |
| `GC_DUNGEON` | `SetDungeonDestinationPosition` (mini map arrow) | state only |
| `GC_LOVER_INFO`, `GC_LOVE_POINT_UPDATE` | marriage gauge | state only |
| `GC_AFFECT_ADD/REMOVE` | `uiaffectshower.py` (buff icons with timers) | icons, names and time left shown; the auto-potion gauge image and the horse/lover icons are not |
| `GC_FISHING` | fishing motions and `OnFishing*` notices | remote players' motions play; the main character cannot fish (no `CG_FISHING`, no rod motion mode), notices not shown |
| `GC_WHISPER` | `uiwhisper.py` (whisper windows) | decoded to `NetEvent::Whisper`, then dropped: whispers are never shown |
| `GC_PVP` | duel/revenge buttons on the target board | keys and challenge/revenge sets kept; target board has no PvP buttons, no `CG` duel request |

## Server commands (`game.py` `__ServerCommand_Build`)

Chat packets of type 5 (`CHAT_TYPE_COMMAND`) are ignored. Not handled:
`DayMode`/`PRESERVE_DayMode` (night environment), `CloseRestartWindow`,
`OpenPrivateShop`, `PartyHealReady`, `ShowMeSafeboxPassword`, `CloseSafebox`,
`ShowMeMallPassword`, `CloseMall`, `RefineSuceeded`/`RefineFailed`,
`xmas_*`/`newyear_boom` effects, `PartyRequest`/`PartyRequestDenied`,
`horse_state`/`hide_horse_state` (horse gauge), `WarUC`, `lover_*`,
`MyShopPriceList`, `ConsoleEnable`. (`item_mall`, `mall` and `PlayMusic`
have partial coverage.)

## Windows and interface

- **Whisper windows**, **party window**, **guild window**, **messenger**, **exchange**,
  **refine dialog**, **equipment inspect**, **private shop builder**, **dragon soul**
  and **cube** windows: none exist (see the packets above).
- **Affect shower**: skill buffs, states and item/premium affects show in the
  top-left with tooltips. Missing: auto-potion fill (`AutoPotionImage`), the
  horse state and love point icons, `PlayTimeImage`.
- **System and game options** (`uisystemoption.py`, `uigameoption.py`): no
  window; always-show-names, damage numbers, PK mode selector and sound
  volumes cannot be changed in game.
- **Pick money / drop Yang** (`uipickmoney.py`), **point reset**, **help**,
  **select item**, **attach metin** (`uiattachmetin.py`, socket a stone by drag),
  **map name shower** (`uimapnameshower.py`) — missing.
- **Skill window**: active and support pages now follow `uicharacter.py`
  (all skills shown, `+` on level 0, pick-up/drag to the task bar, right or
  double click uses). Still missing: the horse/Riding skill page
  (`PAGE_HORSE`), toggled-skill
  highlight (`ActivateSlot`), skill-book tooltips, group names instead of
  "Group1/2" (`SKILL_GROUP_NAME_DICT`), and the `isRequirement` check of
  `CanLevelUpSkill`.
- **Item tooltips**: now built from `AddItemData` (limits, attack/defence,
  applies, bonuses, sockets, wearable jobs, sell price). Missing: accessory
  and belt socket lines, polymorph/skill-book/rod/pick/lottery/blend/unique
  special cases, the hair preview image, socket slot images, costume time
  left, and tooltips for shop/safebox/exchange slots (only inventory,
  equipment and task bar items have them).
- **Task bar**: follows `uitaskbar.py` (left click picks a slot up, right click
  and the number keys use it, dropping on the world removes it); skills show
  their level and cooldown. The cooldown is a top-down cover, not the C++
  clock sweep (`RenderCoolTimeBox`), and there is no finish flash
  (`CCoolTimeFinishEffect`). Dropping a bag item on the world puts it back
  instead of asking to drop it (`__PutItem`).
- **Text tails**: chat bubbles over heads (`RegisterChatTail`) and private
  shop signs are not drawn; item names now show within 3500 units with the
  owner line.
- **Mouse cursor** shapes per hover target (`PythonApplicationCursor.cpp`) —
  check coverage; the attack/talk/pick cursors are not all present.
- **Emoticons over heads** (`SetEmoticon`, e.g. the fishing bite icon).

## Gameplay and simulation

- **Main character fishing and mining** (`__OnClickActor` with a rod/pick,
  `MOTION_MODE_FISHING`, `CG_FISHING`, `CG_DIG`).
- **Horse skills and horse state gauge**, mounted dust effects
  (`EFFECT_HORSE_DUST`, `__IsInDustRange`) and walking dust.
- **Remote monsters** now use `StateProcess`/`MovementProcess` like players,
  but walk at the pace their packet implies (distance over `dwDuration`)
  rather than their run motion's accumulation. Pushes are a separate
  offset, as in C++.
- **`CActorInstance` push collision**: `__Push` from `GC_SYNC_POSITION` now
  blends like C++, but without `IsResistFallen` and without the per-step
  `isPhysicalCollision` test.
- **Target lock** after a hit now follows `SetTarget(vid, FALSE)` (1 s);
  `CanChangeTarget` while casting is not checked.
- **Night/day environment** (`DayMode`), weather and seasonal effects.
- **Sounds**: motion `.mss` sounds and map ambience are not fully wired
  (see `rust_port.md`).
- **Polymorph**: players with a monster race render as that monster (remote
  and main) with its wait/run/attack/damage/dead motions. Not ported: the
  polymorph attack data (`IsPoly` in `InstanceBaseBattle`), polymorph item
  tooltips. **Costume hair** (hair 5001+) and **pets** (npclist races) go
  through the normal hair and NPC paths and need checking in game.
- **Stealth** blends instantly (C++ fades over a second, `BlendAlphaValue`).
- **Guild marks** (`GuildMarkDownloader`, `MarkManager`) over names and on
  the guild war scoreboard.
- **Armor specular** is applied to the main character and remote players
  but not to the character-select preview.

## Recently fixed in this pass (for reference)

Remote player movement (command queue + `MovementProcess`), mounted remote
players sinking, the rider floating above the saddle until the first move,
remote riders' horses running while they attack, PvP hit reactions from
non-hostile players (`IsAttackableInstance` + safe zones + `GC_PVP` /
guild-war keys), weapon/armor refine shine (sphere-map specular),
`GC_SYNC_POSITION` push blending, ground item names, item stat tooltips and
the skill window.

Second pass: skill cooldowns and level counts on the skill page and task
bar, task bar skill tooltips, C++ mouse handling (smart left button with
press/hold/release, Ctrl attack, right/middle camera drag with a pinned
hidden cursor, right-click targeting), the task bar's pick-up/use split,
HP/SP recovery bars, own stealth at half opacity and others hidden, the
affect shower, monsters on the command queue with additive pushes, and
polymorph bodies.
