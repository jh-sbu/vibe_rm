Scriptname VrmAmmoTestScript extends Quest
; test-mod ammo: which ammunition the game equips with a bow or crossbow
; (plans/bow-ammunition.md, step 0). ammo.rs documents the readings.

Actor Property Player Auto
Container Property Chest Auto
Weapon Property LongBow Auto
Weapon Property HuntingBow Auto
Ammo Property Iron Auto
Ammo Property Steel Auto
Ammo Property Elven Auto
Ammo Property TieA Auto
Ammo Property TieB Auto
Ammo Property Pricey Auto

Bool Ran = false
ObjectReference Stash
Weapon Crossbow
Ammo BoltSteel
Ammo BoltDwarven
Form[] Items
String[] Labels

String ResultSetup
String ResultDefault
String ResultChosen
String ResultSwitch
String ResultRemoveHeld
String ResultRemoveAway
String ResultOnlyBolts
String ResultPickup
String ResultIncompatible
String ResultTie
String ResultPricey
String ResultCrossbow
String ResultMenu
String ResultRunOut

Event OnInit()
    RegisterForSingleUpdate(5.0)
EndEvent

Event OnUpdate()
    If Ran
        return
    EndIf
    Ran = true
    Debug.MessageBox("vibe_rm ammo: automatic tests first (about 1 minute), then two short steps for you. Your inventory goes into a barrel beside you for the run and comes back at the end.")
    Utility.Wait(1.0)
    ResultSetup = Setup()
    ResultDefault = TestDefault()
    ResultChosen = TestChosen()
    ResultSwitch = TestSwitch()
    ResultRemoveHeld = TestRemoveHeld()
    ResultRemoveAway = TestRemoveAway()
    ResultOnlyBolts = TestOnlyBolts()
    ResultPickup = TestPickup()
    ResultIncompatible = TestIncompatible()
    ResultTie = TestTie()
    ResultPricey = TestPricey()
    ResultCrossbow = TestCrossbow()
    Game.RequestSave()
    Utility.Wait(3.0)
    ResultMenu = TestMenu()
    ResultRunOut = TestRunOut()
    Restore()
    Game.RequestSave()
    Utility.Wait(3.0)
    Debug.MessageBox("vibe_rm ammo done: your items are back. Read the reports with vrm-tool test-results.")
EndEvent

; The player's own items into a barrel placed beside them, the test items
; and Dawnguard's crossbow and bolts listed.
String Function Setup()
    Stash = Player.PlaceAtMe(Chest, 1, false, false)
    Player.RemoveAllItems(Stash, false, false)
    Crossbow = Game.GetFormFromFile(0x801, "Dawnguard.esm") as Weapon
    BoltSteel = Game.GetFormFromFile(0xBB3, "Dawnguard.esm") as Ammo
    BoltDwarven = Game.GetFormFromFile(0xD099, "Dawnguard.esm") as Ammo
    Items = new Form[11]
    Labels = new String[11]
    Items[0] = Iron
    Labels[0] = "iron"
    Items[1] = Steel
    Labels[1] = "steel"
    Items[2] = Elven
    Labels[2] = "elven"
    Items[3] = TieA
    Labels[3] = "tieA"
    Items[4] = TieB
    Labels[4] = "tieB"
    Items[5] = Pricey
    Labels[5] = "pricey"
    Items[6] = BoltSteel
    Labels[6] = "boltSteel"
    Items[7] = BoltDwarven
    Labels[7] = "boltDwarven"
    Items[8] = LongBow
    Labels[8] = "longBow"
    Items[9] = HuntingBow
    Labels[9] = "huntingBow"
    Items[10] = Crossbow
    Labels[10] = "crossbow"
    String r = "Setup: player items stored: " + (Stash != None)
    r += "; Dawnguard crossbow found: " + (Crossbow != None)
    r += ", bolts: " + (BoltSteel != None) + " " + (BoltDwarven != None) + "\n"
    return r + "Readings: item count, E = IsEquipped.\n"
EndFunction

; What the player carries of the test items: "iron 10 E, steel 10".
String Function State()
    String r = ""
    Int i = 0
    While i < Items.Length
        If Items[i] != None
            Int n = Player.GetItemCount(Items[i])
            If n > 0
                If r != ""
                    r += ", "
                EndIf
                r += Labels[i] + " " + n
                If Player.IsEquipped(Items[i])
                    r += " E"
                EndIf
            EndIf
        EndIf
        i += 1
    EndWhile
    If r == ""
        return "(nothing)"
    EndIf
    return r
EndFunction

; A labelled reading after a second for the game to settle.
String Function Read(String label)
    Utility.Wait(1.0)
    String r = label + ": " + State()
    Debug.Trace("vibe_rm ammo " + r)
    return "  " + r + "\n"
EndFunction

; Every test item taken away (worn ones unequipped first).
Function Clear()
    Int i = 0
    While i < Items.Length
        If Items[i] != None
            If Player.IsEquipped(Items[i])
                Player.UnequipItem(Items[i], false, true)
            EndIf
            Int n = Player.GetItemCount(Items[i])
            If n > 0
                Player.RemoveItem(Items[i], n, true, None)
            EndIf
        EndIf
        i += 1
    EndWhile
    Utility.Wait(0.5)
EndFunction

Function Give(Form f, Int n)
    Player.AddItem(f, n, true)
EndFunction

; f equipped by script, given first if not carried.
Function Equip(Form f)
    If Player.GetItemCount(f) == 0
        Give(f, 1)
    EndIf
    Player.EquipItem(f, false, true)
EndFunction

; Three arrows carried (added steel, elven, iron: not in damage order), the
; bow equipped by script.
String Function TestDefault()
    Clear()
    Give(Steel, 10)
    Give(Elven, 10)
    Give(Iron, 10)
    String r = "Default (steel, elven, iron added; Papyrus EquipItem):\n"
    r += Read("carried")
    Equip(LongBow)
    r += Read("long bow equipped")
    return r
EndFunction

; Steel chosen while the bow is held; the bow taken off and put back.
String Function TestChosen()
    String r = "Chosen (from Default's state):\n"
    Equip(Steel)
    r += Read("steel equipped")
    Player.UnequipItem(LongBow, false, true)
    r += Read("bow unequipped")
    Equip(LongBow)
    r += Read("bow equipped again")
    Player.UnequipItem(LongBow, false, true)
    Player.UnequipItem(Steel, false, true)
    r += Read("bow and steel unequipped")
    Equip(LongBow)
    r += Read("bow equipped, no arrows worn before")
    return r
EndFunction

; Steel worn, the long bow swapped for the hunting bow straight away.
String Function TestSwitch()
    String r = "Switch (steel worn, long bow held):\n"
    Equip(Steel)
    r += Read("steel equipped")
    Equip(HuntingBow)
    r += Read("hunting bow equipped")
    return r
EndFunction

; The worn stack removed while the bow is held: does another equip itself?
String Function TestRemoveHeld()
    Clear()
    Give(Iron, 10)
    Give(Steel, 10)
    Give(Elven, 10)
    Equip(LongBow)
    Equip(Steel)
    String r = "RemoveHeld (bow held, steel worn):\n"
    r += Read("before")
    Player.RemoveItem(Steel, 10, true, None)
    r += Read("steel removed")
    Utility.Wait(2.0)
    r += Read("2 s later")
    Player.UnequipItem(LongBow, false, true)
    Equip(LongBow)
    r += Read("bow equipped again")
    return r
EndFunction

; The worn stack removed with no bow held, then the bow equipped.
String Function TestRemoveAway()
    Clear()
    Give(Iron, 10)
    Give(Steel, 10)
    Give(Elven, 10)
    Equip(Steel)
    String r = "RemoveAway (no bow, steel worn):\n"
    r += Read("before")
    Player.RemoveItem(Steel, 10, true, None)
    r += Read("steel removed")
    Equip(LongBow)
    r += Read("bow equipped")
    return r
EndFunction

; Only bolts carried.
String Function TestOnlyBolts()
    Clear()
    If BoltSteel == None
        return "OnlyBolts: no Dawnguard bolts.\n"
    EndIf
    Give(BoltSteel, 10)
    String r = "OnlyBolts:\n"
    Equip(LongBow)
    r += Read("bow equipped, steel bolts carried")
    return r
EndFunction

; The bow held with nothing to shoot, then arrows picked up.
String Function TestPickup()
    Clear()
    Equip(LongBow)
    String r = "Pickup (bow held, no arrows):\n"
    r += Read("bow equipped")
    Give(Steel, 10)
    r += Read("steel added")
    Give(Iron, 10)
    r += Read("iron added")
    return r
EndFunction

; Bolts worn, arrows carried, the bow equipped.
String Function TestIncompatible()
    Clear()
    If BoltSteel == None
        return "Incompatible: no Dawnguard bolts.\n"
    EndIf
    Give(BoltSteel, 10)
    Give(Steel, 10)
    Give(Iron, 10)
    Equip(BoltSteel)
    String r = "Incompatible (bolts worn, iron and steel carried):\n"
    r += Read("bolts equipped")
    Equip(LongBow)
    r += Read("bow equipped")
    return r
EndFunction

; Two arrows alike but in name and form: tieA (the lower form id, named
; "Vrm Tie Zulu"), tieB ("Vrm Tie Alpha"), added in either order.
String Function TestTie()
    Clear()
    Give(TieA, 10)
    Give(TieB, 10)
    String r = "Tie (iron's damage; A = lower id, named Zulu):\n"
    Equip(LongBow)
    r += Read("A added first, bow equipped")
    Clear()
    Give(TieB, 10)
    Give(TieA, 10)
    Equip(LongBow)
    r += Read("B added first, bow equipped")
    return r
EndFunction

; pricey: damage 4 (below iron's 8), value 100 (iron's 1).
String Function TestPricey()
    Clear()
    Give(Iron, 10)
    Give(Pricey, 10)
    String r = "Pricey (damage 4, value 100) and iron (8, 1):\n"
    Equip(LongBow)
    r += Read("bow equipped")
    return r
EndFunction

String Function TestCrossbow()
    Clear()
    If Crossbow == None || BoltSteel == None || BoltDwarven == None
        return "Crossbow: Dawnguard isn't loaded.\n"
    EndIf
    Give(BoltDwarven, 10)
    Give(BoltSteel, 10)
    Give(Iron, 10)
    String r = "Crossbow (dwarven and steel bolts, iron arrows):\n"
    Equip(Crossbow)
    r += Read("crossbow equipped")
    Equip(LongBow)
    r += Read("long bow equipped")
    Equip(Crossbow)
    r += Read("crossbow equipped again")
    Player.UnequipItem(Crossbow, false, true)
    Equip(BoltDwarven)
    Equip(LongBow)
    r += Read("dwarven bolts worn, then the bow")
    return r
EndFunction

; Waits until the game is out of menus.
Function OutOfMenus()
    While Utility.IsInMenuMode()
        Utility.Wait(0.2)
    EndWhile
EndFunction

; The bow equipped from the inventory menu by the player.
String Function TestMenu()
    Clear()
    Give(Steel, 10)
    Give(Elven, 10)
    Give(Iron, 10)
    Give(LongBow, 1)
    Debug.MessageBox("Step 1 of 2: open the inventory, equip the Long Bow, look at which arrows show as equipped, then close the inventory.")
    OutOfMenus()
    Float waited = 0.0
    While !Player.IsEquipped(LongBow) && waited < 300.0
        Utility.Wait(0.5)
        waited += 0.5
    EndWhile
    OutOfMenus()
    If !Player.IsEquipped(LongBow)
        return "Menu: the bow wasn't equipped within 300 s.\n"
    EndIf
    return "Menu (steel, elven, iron added; equipped by the player):\n" + Read("after the menu")
EndFunction

; Two iron arrows worn with steel and elven carried: three shots.
String Function TestRunOut()
    Clear()
    Give(Iron, 2)
    Give(Steel, 5)
    Give(Elven, 5)
    Equip(LongBow)
    Equip(Iron)
    String r = "RunOut (iron 2 worn; steel, elven 5 carried):\n"
    r += Read("before")
    Debug.MessageBox("Step 2 of 2: fire three arrows with the bow (into the ground is fine), waiting a second between shots. Note any message at the top left. Then wait for the done box.")
    OutOfMenus()
    Int start = 12
    Int shots = 0
    Float waited = 0.0
    While shots < 3 && waited < 300.0
        Utility.Wait(0.2)
        waited += 0.2
        Int n = Player.GetItemCount(Iron) + Player.GetItemCount(Steel) + Player.GetItemCount(Elven)
        If start - n > shots
            shots = start - n
            r += Read("after shot " + shots)
        EndIf
    EndWhile
    If shots < 3
        r += "  only " + shots + " shots within 300 s\n"
    EndIf
    return r
EndFunction

; The test items gone, the player's own back.
Function Restore()
    Clear()
    Stash.RemoveAllItems(Player, false, true)
    Utility.Wait(1.0)
    Stash.Disable(false)
    Stash.Delete()
EndFunction
