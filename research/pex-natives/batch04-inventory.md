# Batch 4: inventory queries

Public references consulted 2026-10-24:
- https://papyrus.bellcube.dev/skyrimse/script/objectreference/function/getallitemscount/
- https://papyrus.bellcube.dev/skyrimse/script/objectreference/ (IsContainerEmpty contract).
- https://papyrus.bellcube.dev/skyrimse/script/actor/function/getgoldamount/
- https://papyrus.bellcube.dev/skyrimse/script/actor/function/wornhaskeyword/
- https://papyrus.bellcube.dev/skyrimse/script/actor/function/getequippedarmorinslot/
- https://elderscrolls.bethesda.net/de-DE/news/4ORD7tshfmHUN8H4ULSgE4/the-elder-scrolls-v-skyrim-special-edition-creations-update-patch-notes
  (official publication of GetAllItemsCount and IsContainerEmpty).

The Papyrus Index attributes wiki text to the public community CK wiki.
GetGoldAmount counts carried gold, not merchant-chest funds. Worn queries use
current equipped items, with slot IDs 30..61 converted to BODT/BOD2 bits;
multi-slot armor is returned for either occupied slot. Inventory counts sum
positive stack quantities with saturation, including gold and equipped items.
That quantity-versus-distinct-form interpretation is an implementation choice.

Local UESP consulted first: ARMO revision 2308925,
https://en.uesp.net/w/index.php?oldid=2308925 (BODT/BOD2, KWDA).
No assets copied or engine binaries inspected; synthetic tests only.
