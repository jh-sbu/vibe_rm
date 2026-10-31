; Odds and ends in the test cell: hands and the graph, stolen goods dropped,
; drinking while invisible.

String Function TestHandTypes()
    Actor a = Spawn(NakedBase, 0.0, 300.0)
    a.SetDontMove(true)
    Utility.Wait(1.0)
    a.EquipSpell(FlamesSpell, 1)
    Utility.Wait(1.0)
    String r = "Flames in the right hand only: GetEquippedItemType left " + a.GetEquippedItemType(0) + ", right " + a.GetEquippedItemType(1)
    r += "; graph iLeftHandType " + a.GetAnimationVariableInt("iLeftHandType") + ", iRightHandType " + a.GetAnimationVariableInt("iRightHandType")
    a.SetAlert(true)
    a.DrawWeapon()
    Utility.Wait(2.0)
    r += "; drawn: " + a.GetAnimationVariableInt("iLeftHandType") + "/" + a.GetAnimationVariableInt("iRightHandType") + "\n"
    a.UnequipSpell(FlamesSpell, 1)
    a.AddItem(Dagger, 1, true)
    a.EquipItem(Dagger, false, true)
    Utility.Wait(1.0)
    a.DrawWeapon()
    Utility.Wait(2.0)
    r += "a dagger (speed 1.3) drawn: weaponSpeedMult " + Num(a.GetAnimationVariableFloat("weaponSpeedMult")) + ", leftWeaponSpeedMult " + Num(a.GetAnimationVariableFloat("leftWeaponSpeedMult"))
    a.SetActorValue("WeaponSpeedMult", 2.0)
    Utility.Wait(1.0)
    r += "; WeaponSpeedMult 2: " + Num(a.GetAnimationVariableFloat("weaponSpeedMult")) + "\n"
    a.SetAlert(false)
    Gone(a)
    return r
EndFunction

; crime.md: a stolen item dropped and picked up again, checked by paying a
; bounty with "remove stolen items".
String Function TestStolenDrop()
    ObjectReference o = Put(IngotIron, 0.0, 150.0, 20.0)
    o.SetFactionOwner(CrimeFaction)
    Utility.Wait(0.5)
    Int before = Player.GetItemCount(IngotIron)
    o.Activate(Player, false)
    Utility.Wait(1.0)
    String r = "an ingot owned by the crime faction taken: ingots " + (Player.GetItemCount(IngotIron) - before) + ", bounty " + CrimeFaction.GetCrimeGold()
    ObjectReference d = Player.DropObject(IngotIron, 1)
    Utility.Wait(1.0)
    d.Activate(Player, false)
    Utility.Wait(1.0)
    r += "; dropped and picked up: ingots " + (Player.GetItemCount(IngotIron) - before) + ", bounty " + CrimeFaction.GetCrimeGold()
    CrimeFaction.SetCrimeGold(10)
    Player.AddItem(Gold, 100, true)
    CrimeFaction.PlayerPayCrimeGold(true, false)
    Utility.Wait(1.0)
    r += "; bounty paid with stolen goods removed: ingots " + (Player.GetItemCount(IngotIron) - before) + "\n"
    Player.RemoveItem(IngotIron, Player.GetItemCount(IngotIron) - before, true, None)
    CrimeFaction.SetCrimeGold(0)
    return r
EndFunction

; invisibility.md: drinking a potion while invisible.
String Function TestInvisiblePotion()
    InvisibilitySpell.Cast(Player, Player)
    Utility.Wait(1.0)
    String r = "Invisibility on the player: value " + Player.GetActorValue("Invisibility")
    Player.AddItem(PotionHealth, 1, true)
    Player.EquipItem(PotionHealth, false, true)
    Utility.Wait(1.0)
    r += "; a potion drunk (EquipItem): " + Player.GetActorValue("Invisibility")
    Player.AddItem(SweetRoll, 1, true)
    Player.EquipItem(SweetRoll, false, true)
    Utility.Wait(1.0)
    r += "; food eaten: " + Player.GetActorValue("Invisibility")
    ObjectReference c = Put(EmptyChest, 0.0, 150.0, 0.0)
    c.Lock(true, false)
    c.SetLockLevel(255)
    c.Activate(Player, false)
    Utility.Wait(1.0)
    r += "; Activate(player) on a key-only chest: " + Player.GetActorValue("Invisibility") + "\n"
    Player.DispelSpell(InvisibilitySpell)
    Gone(c)
    return r
EndFunction
