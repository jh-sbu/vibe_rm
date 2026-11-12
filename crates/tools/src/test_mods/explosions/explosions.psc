; Explosions placed with PlaceAtMe in the test cell, away from the player
; (CellEntry is 900 west of CellCenter). Round 2 (round 1's readings are in
; research/test-mod-explosions.md): line of sight with the explosion above
; the floor, and knockdowns of NPCs left free.

; The Story Manager log the story quests write to (VrmAt21StoryLog).
String StoryLog = ""

Function AddStory(String s)
    StoryLog = StoryLog + s + "; "
EndFunction

; A logged NPC with 1000 more health, held where it is put unless free.
Actor Function Target(Float x, Float y)
    return Placed(x, y, true)
EndFunction

Actor Function Placed(Float x, Float y, Bool held)
    Actor a = Spawn(LoggedBase, x, y)
    a.SetDontMove(held)
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

; Shade, again: the thrall's fireball (damage 60 and its fire, radius
; 320, line of sight checked) placed at a marker `z` above the floor: an
; NPC 90 north, one 200 north behind it, one 200 east in the open.
String Function ShadeAt(Float z)
    ObjectReference spot = Put(XMarker, 0.0, 600.0, z)
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
    String s = "at " + Num(z) + " up (spot " + Num(spot.GetPositionZ() - CellCenter.GetPositionZ()) + " above the centre): 90 north " + Took(shield, b0) + "; 200 north, behind it " + Took(behind, b1) + "; 200 east, open " + Took(open, b2) + "\n"
    Gone(shield)
    Gone(behind)
    Gone(open)
    Gone(spot)
    Utility.Wait(1.0)
    return s
EndFunction

String Function TestShadeLifted()
    GoCell()
    return "thrall fireball, health lost [events] after 4 s:\n" + ShadeAt(0.0) + ShadeAt(60.0) + ShadeAt(120.0)
EndFunction

; Knockdowns, again, the NPC left free (round 1's were held by
; SetDontMove and none moved): Always at full health, By Formula at 100%,
; 60%, 45% and 20%, and the ballista explosion (no knockdown flag, force
; 20, damage 80) as a control. Read at once through the probe, and how far
; it moved.
String Function FreeKnock(Explosion x, Float fraction, String label)
    Actor a = Placed(0.0, 700.0, false)
    Utility.Wait(1.5)
    a.DamageActorValue("Health", a.GetActorValueMax("Health") * (1.0 - fraction))
    Float left = a.GetActorValuePercentage("Health")
    Float x0 = a.GetPositionX()
    Float y0 = a.GetPositionY()
    Float z0 = a.GetPositionZ()
    ObjectReference spot = Put(XMarker, a.GetPositionX() - CellCenter.GetPositionX() - 50.0, a.GetPositionY() - CellCenter.GetPositionY(), 30.0)
    spot.PlaceAtMe(x, 1, false, false)
    Utility.Wait(0.3)
    Float dx = a.GetPositionX() - x0
    Float dy = a.GetPositionY() - y0
    Float dz = a.GetPositionZ() - z0
    String r = "  " + label + " at " + Num(left) + ": knocked " + Probe(a, PS_Knocked, PE_Knocked) + ", moved " + Num(Math.sqrt(dx * dx + dy * dy)) + " (z " + Num(dz) + ") in 0.3 s, health now " + Num(a.GetActorValuePercentage("Health")) + "\n"
    Gone(a)
    Gone(spot)
    Utility.Wait(1.0)
    return r
EndFunction

String Function TestFreeKnockdowns()
    String s = "knockdowns of a free NPC, the explosion 50 west of it, 30 up:\n"
    s += FreeKnock(ExplBallista, 1.0, "ballista (no flag)")
    s += FreeKnock(ExplAlways, 1.0, "Always")
    s += FreeKnock(ExplByFormula, 1.0, "By Formula")
    s += FreeKnock(ExplByFormula, 0.6, "By Formula")
    s += FreeKnock(ExplByFormula, 0.45, "By Formula")
    s += FreeKnock(ExplByFormula, 0.2, "By Formula")
    return s
EndFunction
