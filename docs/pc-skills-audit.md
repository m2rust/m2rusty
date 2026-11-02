# PC skill parity audit

Checked against `client/src/ui/PythonPlayerSkill.cpp`,
`client/src/ui/PythonPlayerEventHandler.cpp`, `client/src/game/ActorInstance*.cpp`,
`bin/pack/root/playersettingmodule.py`, and the English client packs.

## Coverage

| Class | First group | Second group | Skills |
|---|---|---|---:|
| Warrior | 1–5 | 16–20 | 10 |
| Assassin | 31–35 | 46–50 | 10 |
| Sura | 61–66 | 76–81 | 12 |
| Shaman | 91–96 | 106–111 | 12 |

The packed-data regression checks all 44 available class skills for both body
variants and all four grades: 352 skill/race/grade combinations. It validates
motion registration, animation availability, effect/projectile files, splash
spheres, and formula-derived shot/target limits. The English locale does not
register the additional YMIR-only warrior/assassin sixth skills.

Mounted skill motions (121–123) and guild motions (101–106) are also checked
against each race's Python registrations and supported mounted weapon modes.

## Corrected shared behavior

- Fly events send `CG_SHOOT` with the playing skill's ID; `CG_USE_SKILL` and
  targeting alone do not deliver projectile damage. Server damage and splash
  calculations remain authoritative.
- Skill formulas use the international skill-power table, including mastery
  offsets. `floor` in repeated-shot/target-count formulas is evaluated.
- Motion LoopData repeats its section, replays shot events, and then plays the
  outro. Additional fan targets are queued, including empty shots when fewer
  enemies exist than the requested count.
- Self/allied buffs resolve the main actor as a target. Grade-zero shaman buffs
  use `_target.msa`; guild buffs use `skill/`; horse charge uses `horse/`.
- Dash first sends a targetless use request, retains its reservation, accepts
  server movement-speed updates, and completes against the target while the
  dash affect bypasses the initial SP/cooldown checks.
- Casting clears old click actions. Ambush's zero accumulation therefore stays
  in place after the cast. Moving horse skills retain movement.
- Aura of the Sword follows the correct weapon bone, chooses sword/spear
  effects, and extends the weapon sweep to 1.5×.
- Remote swings and skill splashes produce hit effects/reactions against mobs
  and players without sending another player's attack packets. Great hits can
  interrupt attacks; knockdown/stand-up sequences protect their animation from
  queued movement. Strong Body and mounts resist falling.
- Motion effects follow their attachments; target effects, hide/show, warp,
  and screen-wave events are handled. Short effects wait for their textures
  before advancing their clocks. Persistent markers/affects can restart after
  expiry while still active.
- Active toggle skills can be turned off during cooldown using server affect
  flags, and unlearned skills are rejected.

## Related fixes

Quest-letter scripts update quest state without opening an empty modal. Quest
NPCs/locations receive the original red-arrow world effect and flashing
4-pixel minimap marks, with distant marks clamped to the minimap edge.
Camera drag uses a locked cursor and relative mouse motion without pointer
warps, avoiding Wayland's cursor-position errors.

## Validation limits

Automated tests cover packed assets, state transitions, real outgoing packet
bytes, loop/event timing, and rendering texture readiness. They do not replace
live server checks for damage amounts, poison/buff application, PvP rules, or
visual timing in combat. YMIR-only skills require their own locale/server data.
