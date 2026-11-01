; autotest21's small faults, again in the test cell.

; query-natives: IsUnique on a base flagged unique with no template (all of
; autotest21's had the commoner as a traits template).
String Function TestUniquePlain()
    String r = "unique flag, no template: IsUnique " + UniquePlainBase.IsUnique()
    r += "; the same flag under a non-unique traits template: " + UniqueBase.IsUnique()
    r += "; Gerdur (unique, vanilla): " + GerdurBase.IsUnique() + "\n"
    return r
EndFunction

; disarm: autotest21's Disarm did nothing to a level 10 NPC. Now a level 1
; one, sword drawn and in combat with a held target, and the level 10 one
; again as a control.
String Function DisarmOne(ActorBase b, String label)
    Actor a = Spawn(b, 0.0, 250.0)
    Actor t = Spawn(InvulnerableBase, 0.0, 450.0)
    t.SetDontMove(true)
    a.AddItem(IronSword, 1, true)
    a.EquipItem(IronSword, false, true)
    Utility.Wait(1.0)
    a.StartCombat(t)
    Utility.Wait(3.0)
    (a as VrmAt21ActorLog).Take()
    String r = label + ": drawn " + a.IsWeaponDrawn() + ", combat " + a.GetCombatState()
    DisarmSpell.Cast(Player, a)
    Utility.Wait(2.0)
    r += "; after Disarm: holds " + Id(a.GetEquippedWeapon(false)) + ", count " + a.GetItemCount(IronSword) + ", events " + (a as VrmAt21ActorLog).Take()
    ObjectReference s = Game.FindClosestReferenceOfTypeFromRef(IronSword, a, 1000.0)
    If s != None && s.GetParentCell() == a.GetParentCell()
        r += "; a sword on the ground: dx " + Num(s.GetPositionX() - a.GetPositionX()) + ", dy " + Num(s.GetPositionY() - a.GetPositionY()) + ", dz " + Num(s.GetPositionZ() - a.GetPositionZ())
    EndIf
    a.StopCombat()
    Gone(a)
    Gone(t)
    Gone(s)
    return r + "\n"
EndFunction

String Function TestDisarm()
    return DisarmOne(LowLoggedBase, "a level 1 NPC") + DisarmOne(LoggedBase, "a level 10 NPC")
EndFunction

; invisibility: eating food alone, then activating a locked chest alone.
String Function TestInvisibleFood()
    InvisibilitySpell.Cast(Player, Player)
    Utility.Wait(1.0)
    String r = "Invisibility on the player: " + Player.GetActorValue("Invisibility")
    Player.AddItem(SweetRoll, 1, true)
    Player.EquipItem(SweetRoll, false, true)
    Utility.Wait(1.0)
    r += "; a sweet roll eaten: " + Player.GetActorValue("Invisibility") + "\n"
    Player.DispelSpell(InvisibilitySpell)
    Utility.Wait(1.0)
    return r
EndFunction

String Function TestInvisibleActivate()
    ObjectReference c = Put(EmptyChest, 0.0, 150.0, 0.0)
    c.Lock(true, false)
    c.SetLockLevel(255)
    InvisibilitySpell.Cast(Player, Player)
    Utility.Wait(1.0)
    String r = "Invisibility on the player: " + Player.GetActorValue("Invisibility")
    c.Activate(Player, false)
    Utility.Wait(1.0)
    r += "; Activate(player) on a key-only chest: " + Player.GetActorValue("Invisibility") + "\n"
    Player.DispelSpell(InvisibilitySpell)
    Gone(c)
    return r
EndFunction
