# Form lists changed by scripts

## Sources

- CommonLibSSE `BGSListForm.h` / `.cpp`: a list keeps its plugin forms
  (`forms`, `LNAM`) and, apart, the ids scripts added
  (`scriptAddedTempForms`, `scriptAddedFormCount`), with a change flag
  `kAddedForm` (1 << 31) for saves. `ForEachForm` walks the plugin's forms
  first, then the added ones; `HasForm` looks in both.
- Vanilla `FormList.pex` natives: `AddForm(Form)`, `RemoveAddedForm(Form)`,
  `Revert()`, `Find(Form)`, `GetAt`, `GetSize`, `HasForm`. 245 calls are
  named `AddForm` (shared with the leveled lists', which aren't supported),
  21 `Find`, 8 `Revert`, 4 `RemoveAddedForm` (`vrm-tool pex-natives`).

## What we do

- `AddForm` puts the form after the plugin's and earlier additions, unless
  the list has it already. `RemoveAddedForm` takes out an added form only;
  `Revert` takes out all of them. `GetAt`, `GetSize`, `HasForm`, `Find` (-1
  when absent) and everything else reading the list (`IsInList`, gift and
  item filters) see the additions. Saved in optional `FLAD`, by list in
  order added; legacy saves have none.
- NPCs' default package lists (`DPLT`) are still read from the plugin, so a
  package added there by script isn't seen.

## Unconfirmed

- Where `GetAt` finds the added forms. We follow `ForEachForm` (plugin's
  first). Modding folklore has it that script-added forms come first (index
  0 the latest added); no accessible source confirms either.
- Whether `AddForm` adds a form already present a second time (we don't),
  and whether `RemoveAddedForm` on a plugin form does anything (we leave it).
- Whether additions to a list persist in the original's saves for every
  list or only for lists from loaded plugins (the change flag suggests they
  are saved).
