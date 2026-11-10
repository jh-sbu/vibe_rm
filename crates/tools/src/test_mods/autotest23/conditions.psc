; Conditions read through probes (the OneHanded gain), in the test cell:
; autotest22's partial readings, now by bisection on ProbeValue.

; Where a "GE ProbeValue" probe stops holding, between lo (holds) and hi
; (fails), halving steps times.
String Function Bisect(Actor a, Spell s, MagicEffect e, Float lo, Float hi, Int steps)
    ProbeValue.SetValue(lo)
    If Probe(a, s, e) != "yes"
        return "fails already at " + Num(lo)
    EndIf
    ProbeValue.SetValue(hi)
    If Probe(a, s, e) == "yes"
        return "holds even at " + Num(hi)
    EndIf
    Int i = 0
    While i < steps
        Float mid = (lo + hi) / 2.0
        ProbeValue.SetValue(mid)
        If Probe(a, s, e) == "yes"
            lo = mid
        Else
            hi = mid
        EndIf
        i += 1
    EndWhile
    return "in [" + Num(lo) + ", " + Num(hi) + ")"
EndFunction

; GetXPForNextLevel (autotest22: >= 0 held, >= 25 didn't, with about 33 XP
; held): before and after a skill increase, and after skill use alone.
String Function XPState()
    return "level " + Player.GetLevel() + ", Alteration " + Num(Player.GetBaseActorValue("Alteration")) + ", skill increases " + Game.QueryStat("Skill Increases")
EndFunction

String Function TestXPNext()
    String r = XPState() + ": GetXPForNextLevel " + Bisect(Player, PS_XPNextGE, PE_XPNextGE, -1.0, 4000.0, 15) + "\n"
    Game.IncrementSkill("Alteration")
    Utility.Wait(2.0)
    r += "IncrementSkill(Alteration), " + XPState() + ": " + Bisect(Player, PS_XPNextGE, PE_XPNextGE, -1.0, 4000.0, 15) + "\n"
    Game.AdvanceSkill("Alteration", 5.0)
    Utility.Wait(2.0)
    r += "AdvanceSkill(Alteration, 5), " + XPState() + ": " + Bisect(Player, PS_XPNextGE, PE_XPNextGE, -1.0, 4000.0, 15) + "\n"
    return r
EndFunction

; death: GetTimeDead's unit (autotest22: >= 1 held and >= 10 didn't 71 s
; after the kill), read twice with game time raced between; IsKillerObject
; run on the victim with a list of the killer's base, then reference.
Float Function GameHours()
    return Utility.GetCurrentGameTime() * 24.0
EndFunction

String Function TestTimeDead()
    Alpha.SetDontMove(true)
    Victim.SetDontMove(true)
    Alpha.MoveTo(CellCenter, -200.0, -600.0, 0.0, false)
    Victim.MoveTo(CellCenter, 200.0, -600.0, 0.0, false)
    Utility.Wait(1.0)
    Victim.Kill(Alpha)
    Float t0 = Now()
    Float h0 = GameHours()
    Utility.Wait(1.0)
    String r = "Victim killed by Alpha: GetKiller " + Id(Victim.GetKiller()) + " (Alpha " + Id(Alpha) + "); on the victim, IsKillerObject(Alpha's base) " + Probe(Alpha, PS_KillerObjBase, PE_KillerObjBase) + ", (Alpha) " + Probe(Alpha, PS_KillerObjRef, PE_KillerObjRef) + "\n"
    Float a = Now() - t0
    Float ga = GameHours() - h0
    String s = Bisect(Alpha, PS_TimeDead, PE_TimeDead, 0.0, 2000.0, 15)
    r += "GetTimeDead " + s + ", read from " + Num(a) + " to " + Num(Now() - t0) + " real s (" + Num(ga) + " to " + Num(GameHours() - h0) + " game hours) after the kill\n"
    Float ts = TimeScale.GetValue()
    TimeScale.SetValue(1000.0)
    Utility.Wait(10.0)
    TimeScale.SetValue(ts)
    Utility.Wait(1.0)
    a = Now() - t0
    ga = GameHours() - h0
    s = Bisect(Alpha, PS_TimeDead, PE_TimeDead, 0.0, 2000.0, 15)
    r += "timescale 1000 for 10 s, then: " + s + ", read from " + Num(a) + " to " + Num(Now() - t0) + " real s (" + Num(ga) + " to " + Num(GameHours() - h0) + " game hours)\n"
    return r
EndFunction

; condition-geometry: GetRelativeAngle with its axis (autotest22 gave none).
String Function RelCase(Float az, Float bx, Float by, Float bz, String label)
    Alpha.MoveTo(CellCenter, 0.0, 0.0, 0.0, false)
    Alpha.SetAngle(0.0, 0.0, az)
    Beta.MoveTo(CellCenter, bx, by, 0.0, false)
    Beta.SetAngle(0.0, 0.0, bz)
    Utility.Wait(1.0)
    String r = "  " + label + " (angles " + Num(Alpha.GetAngleZ()) + " / " + Num(Beta.GetAngleZ()) + ", native GetHeadingAngle " + Num(Alpha.GetHeadingAngle(Beta)) + "): "
    return r + Bisect(Alpha, PS_RelAngleZ, PE_RelAngleZ, -400.0, 400.0, 9) + "\n"
EndFunction

String Function TestRelAngle()
    Alpha.SetDontMove(true)
    Beta.SetDontMove(true)
    String r = "GetRelativeAngle(Beta, Z) on Alpha:\n"
    r += RelCase(0.0, 300.0, 0.0, 90.0, "Alpha facing north, Beta east facing east")
    r += RelCase(0.0, 300.0, 0.0, 180.0, "Alpha north, Beta east facing south")
    r += RelCase(0.0, 0.0, 300.0, 90.0, "Alpha north, Beta north facing east")
    r += RelCase(0.0, 0.0, 300.0, 270.0, "Alpha north, Beta north facing west")
    r += RelCase(30.0, 300.0, 0.0, 90.0, "Alpha facing 30, Beta east facing east")
    return r
EndFunction

; status conditions: GetDisease on the diseased player (autotest22 read an
; NPC).
String Function TestPlayerDisease()
    String r = "the player without Ataxia: GetDisease " + Probe(Player, PS_Disease, PE_Disease)
    Player.AddSpell(Ataxia, false)
    Utility.Wait(1.0)
    r += "; with it (HasSpell " + Player.HasSpell(Ataxia) + "): " + Probe(Player, PS_Disease, PE_Disease)
    Player.RemoveSpell(Ataxia)
    Utility.Wait(1.0)
    return r + "; removed: " + Probe(Player, PS_Disease, PE_Disease) + "\n"
EndFunction

; item conditions: a torch and a shield drawn, in combat (autotest22's NPC
; sheathed before the probes after a bare DrawWeapon).
String Function DrawnCase(Form offhand, Bool torch, String label)
    Actor n = Spawn(RedFighterBase, 0.0, 250.0)
    Actor t = Spawn(InvulnerableBase, 0.0, 400.0)
    t.SetDontMove(true)
    n.AddItem(offhand, 1, true)
    n.EquipItem(offhand, false, true)
    Utility.Wait(1.0)
    n.StartCombat(t)
    Utility.Wait(3.0)
    String r = label + ": combat " + n.GetCombatState() + ", drawn " + n.IsWeaponDrawn() + ", hand types left " + n.GetEquippedItemType(0) + " right " + n.GetEquippedItemType(1)
    If torch
        r += "; IsTorchOut " + Probe(n, PS_TorchOut, PE_TorchOut) + ", IsActorUsingATorch " + Probe(n, PS_Torch, PE_Torch)
    Else
        r += "; shield " + Id(n.GetEquippedShield()) + ", IsShieldOut " + Probe(n, PS_ShieldOut, PE_ShieldOut)
    EndIf
    r += "; still drawn " + n.IsWeaponDrawn() + ", combat " + n.GetCombatState() + "\n"
    n.StopCombat()
    Gone(n)
    Gone(t)
    return r
EndFunction

String Function TestDrawnHands()
    return DrawnCase(Torch, true, "a fighter with a sword and a torch") + DrawnCase(IronShield, false, "a fighter with a sword and a shield")
EndFunction
