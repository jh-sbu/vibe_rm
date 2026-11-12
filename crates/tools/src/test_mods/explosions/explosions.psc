; Explosions placed with PlaceAtMe in the test cell, away from the player
; (CellEntry is 900 west of CellCenter). Readings for
; known_ambiguities/explosions.md: damage by distance and armor, line of
; sight, OnHit's source and aggressor, knockdowns, the reference's life.

; The Story Manager log the story quests write to (VrmAt21StoryLog).
String StoryLog = ""

Function AddStory(String s)
    StoryLog = StoryLog + s + "; "
EndFunction

; A logged NPC with 1000 more health, held where it is put.
Actor Function Target(Float x, Float y)
    Actor a = Spawn(LoggedBase, x, y)
    a.SetDontMove(true)
    a.ModActorValue("Health", 1000.0)
    a.RestoreActorValue("Health", 2000.0)
    return a
EndFunction

; What a took: health lost since `before`, and its events.
String Function Took(Actor a, Float before)
    return Num(before - a.GetActorValue("Health")) + " [" + (a as VrmAt21ActorLog).Take() + "]"
EndFunction

; The reference PlaceAtMe gives for an explosion, read over 6 s.
String Function RefState(ObjectReference r, ObjectReference spot)
    return "deleted " + r.IsDeleted() + ", enabled " + r.IsEnabled() + ", 3D " + r.Is3DLoaded() + ", " + Num(r.GetDistance(spot)) + " from the spot"
EndFunction

String Function TestPlacedRef()
    GoCell()
    ObjectReference spot = Put(XMarker, 0.0, 600.0, 0.0)
    Utility.Wait(1.0)
    ObjectReference r = spot.PlaceAtMe(ExplBallista, 1, false, false)
    String s = "PlaceAtMe(TrapBallistaBoltExplosion01): " + Id(r) + ", base " + Id(r.GetBaseObject()) + "\n  at once: " + RefState(r, spot)
    Utility.Wait(1.0)
    s += "\n  at 1 s: " + RefState(r, spot)
    Utility.Wait(5.0)
    s += "\n  at 6 s: " + RefState(r, spot) + "\n"
    Gone(spot)
    return s
EndFunction

; damage by distance (TrapBallistaBoltExplosion01: damage 80, radius 320,
; ignores line of sight), each NPC on a line north of the spot; one in iron
; armor and shield at 200 to the south.
String Function TestFalloff()
    ObjectReference spot = Put(XMarker, 0.0, 600.0, 0.0)
    Float[] at = new Float[5]
    at[0] = 40.0
    at[1] = 120.0
    at[2] = 200.0
    at[3] = 280.0
    at[4] = 360.0
    Actor[] t = new Actor[5]
    Int i = 0
    While i < 5
        t[i] = Target(0.0, 600.0 + at[i])
        i += 1
    EndWhile
    Actor armored = Target(0.0, 400.0)
    armored.AddItem(IronCuirass, 1, true)
    armored.EquipItem(IronCuirass, false, true)
    armored.AddItem(IronShield, 1, true)
    armored.EquipItem(IronShield, false, true)
    Utility.Wait(2.0)
    Float[] before = new Float[5]
    i = 0
    While i < 5
        (t[i] as VrmAt21ActorLog).Take()
        before[i] = t[i].GetActorValue("Health")
        i += 1
    EndWhile
    (armored as VrmAt21ActorLog).Take()
    Float armoredBefore = armored.GetActorValue("Health")
    String s = "ballista explosion (explosion " + Id(ExplBallista) + "); by distance north, health lost [events]:"
    spot.PlaceAtMe(ExplBallista, 1, false, false)
    Utility.Wait(2.0)
    i = 0
    While i < 5
        s += "\n  " + Num(at[i]) + ": " + Took(t[i], before[i]) + ", now " + Num(t[i].GetDistance(spot)) + " off"
        Gone(t[i])
        i += 1
    EndWhile
    s += "\n  200 south, iron cuirass and shield (armor " + Num(armored.GetActorValue("DamageResist")) + "): " + Took(armored, armoredBefore) + "\n"
    Gone(armored)
    Gone(spot)
    return s
EndFunction

; line of sight: the thrall's fireball (damage 60, its fire enchantment,
; radius 320, line of sight checked). One NPC 200 north behind another at
; 90 north; one 200 east in the open.
String Function TestShade()
    ObjectReference spot = Put(XMarker, 0.0, 600.0, 0.0)
    Actor shield = Target(0.0, 690.0)
    Actor behind = Target(0.0, 800.0)
    Actor open = Target(200.0, 600.0)
    Utility.Wait(2.0)
    (shield as VrmAt21ActorLog).Take()
    (behind as VrmAt21ActorLog).Take()
    (open as VrmAt21ActorLog).Take()
    Float b0 = shield.GetActorValue("Health")
    Float b1 = behind.GetActorValue("Health")
    Float b2 = open.GetActorValue("Health")
    spot.PlaceAtMe(ExplThrall, 1, false, false)
    Utility.Wait(4.0)
    String s = "thrall fireball (" + Id(ExplThrall) + "), health lost [events] after 4 s:\n  90 north: " + Took(shield, b0) + "\n  200 north, behind it: " + Took(behind, b1) + "\n  200 east, in the open: " + Took(open, b2) + "\n"
    Gone(shield)
    Gone(behind)
    Gone(open)
    Gone(spot)
    return s
EndFunction

; knockdowns: an explosion placed at an NPC at a fraction of its health:
; "Knock Down - Always" (fakeForceBallLite1024) at full health, "By
; Formula" (fakeForceBall1024ByFormula, no damage) at 100% to 20%, and
; none. GetKnockedStateEnum read through a probe right after.
String Function KnockCase(Explosion x, Float fraction, String label)
    Actor a = Target(0.0, 700.0)
    Utility.Wait(1.5)
    a.DamageActorValue("Health", a.GetActorValueMax("Health") * (1.0 - fraction))
    Float left = a.GetActorValuePercentage("Health")
    If x != None
        a.PlaceAtMe(x, 1, false, false)
    EndIf
    Utility.Wait(0.2)
    String r = "  " + label + " at " + Num(left) + ": knocked " + Probe(a, PS_Knocked, PE_Knocked) + ", moved " + Num(a.GetDistance(CellCenter) - 700.0) + "\n"
    Gone(a)
    Utility.Wait(1.0)
    return r
EndFunction

String Function TestKnockdowns()
    String s = "knockdowns (the player 1080 away):\n"
    s += KnockCase(None, 1.0, "no explosion")
    s += KnockCase(ExplAlways, 1.0, "Always")
    s += KnockCase(ExplByFormula, 1.0, "By Formula")
    s += KnockCase(ExplByFormula, 0.6, "By Formula")
    s += KnockCase(ExplByFormula, 0.45, "By Formula")
    s += KnockCase(ExplByFormula, 0.2, "By Formula")
    return s
EndFunction
