// SPDX-License-Identifier: MPL-2.0
DefinitionBlock ("", "DSDT", 2, "LOGANS", "S5NEG   ", 1)
{
Method (\_S5, 0) { Return (Package (2) { 7, 0 }) }
Scope (\_SB) { Name (_S5, Package (0x02) { Zero, Zero }) }
}
