<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Review #14: P5 power menu

Reviewed head: `73453e318f6677a1e8c018e3ad2b3d46e202d248` (`pr14`), against `pr8`.

- **FAIL — 1.** `userland/neo-init/src/power_view.rs:135-145`: no power call occurs until Enter or the drawn Go button, and Esc or Back cancels. However, other keys match `Input::Key(_) => {}` and leave the confirm card active instead of cancelling. Concrete input: press `a` while the confirm card is shown; the loop continues waiting rather than returning to Home.
- **PASS — 2.** `userland/neo-init/src/desktop_session.rs:3798-3807` recognizes the typed command in neo-init before handing an ordinary nonempty line onward (`:3818-3820`). The sole userland `sys_power` call site is `userland/neo-init/src/power_view.rs:157`, after confirmation; Arthur's reply path does not call it.
- **PASS — 3.** With `power-menu` disabled, the special Home route is compiled out (`userland/neo-init/src/desktop_session.rs:3799`), so the line follows the normal intent path (`:3818-3820`). Product image kernel features include `power-cap` (`tools/xtask/src/main.rs:6493-6498`), the product neo-init feature list includes `power-menu` (`:7093-7095`), and `product_image_pairs_power_menu_with_power_cap` checks both (`:24087-24099`).
- **PASS — 4.** The serial script waits for Home input and then the displayed confirm before sending the confirming Enter (`tools/xtask/src/main.rs:911-916`). The scenario's serial oracle requires confirm, request, shutdown start, K1, K4, and S5 in order (`:11255-11268`); its QMP expectation is `PowerOff` (`:11135-11146`). Skipping the confirm prevents the script from reaching its second Enter step, and reordered/missing markers fail the ordered check.
