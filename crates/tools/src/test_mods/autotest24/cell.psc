; autotest23's faults in the test cell: GetDisease, the view cone, Disarm.

; The Story Manager log the story quests write to (VrmAt21StoryLog).
String StoryLog = ""

Function AddStory(String s)
    StoryLog = StoryLog + s + "@" + Math.Floor(Utility.GetCurrentRealTime() * 100.0) + "; "
EndFunction

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

; status conditions: what makes GetDisease true (autotest23: Ataxia added
; by AddSpell didn't, on the player or an NPC). The disease cast as the
; game's traps give it, cast as itself, and vampirism's.
String Function Diseased(String label)
    Utility.Wait(1.5)
    return label + ": HasSpell(Ataxia) " + Player.HasSpell(Ataxia) + ", GetDisease " + Probe(Player, PS_Disease, PE_Disease)
EndFunction

Function Cure()
    Player.RemoveSpell(Ataxia)
    Player.RemoveSpell(Sanguinare)
    Utility.Wait(1.0)
EndFunction

String Function TestDisease()
    Cure()
    String r = Diseased("none") + "\n"
    TrapAtaxia.Cast(Player, Player)
    r += Diseased("the trap's Ataxia cast on the player") + "\n"
    Cure()
    Ataxia.Cast(Player, Player)
    r += Diseased("Ataxia itself cast on the player") + "\n"
    Cure()
    Actor n = Spawn(NakedBase, 0.0, 150.0)
    n.SetDontMove(true)
    Utility.Wait(1.0)
    TrapAtaxia.Cast(n, Player)
    r += Diseased("the trap's Ataxia cast by an NPC") + "\n"
    Cure()
    Gone(n)
    Player.AddSpell(Sanguinare, false)
    r += Diseased("Sanguinare Vampiris added (HasSpell " + Player.HasSpell(Sanguinare) + ")") + "\n"
    Cure()
    return r + Diseased("all removed") + "\n"
EndFunction

; detection: the view cone's edge, the observer in the middle of the floor
; facing north (autotest23's spots past 90 were off the floor). The player
; is placed from CellCenter, where the observer stands.
String Function SeenAt(Actor o, Float x, Float y)
    Player.MoveTo(CellCenter, x, y, 0.0, false)
    Utility.Wait(2.0)
    String r = "" + Player.IsDetectedBy(o)
    Utility.Wait(3.0)
    return r + "/" + Player.IsDetectedBy(o) + "@" + Num(o.GetDistance(Player))
EndFunction

String Function TestViewCone()
    Actor o = Spawn(NakedBase, 0.0, 0.0)
    o.SetDontMove(true)
    o.MoveTo(CellCenter, 0.0, 0.0, 0.0, false)
    o.SetAngle(0.0, 0.0, 0.0)
    Utility.Wait(1.0)
    Player.StartSneaking()
    Utility.Wait(1.0)
    String r = "observer (level " + o.GetLevel() + ", Sneak " + Num(o.GetActorValue("Sneak")) + ") in the middle facing north; the player sneaking " + Player.IsSneaking() + ", Sneak " + Num(Player.GetActorValue("Sneak")) + ", light " + Num(Player.GetLightLevel()) + "; at 300, by bearing: seen at 2 s/5 s@distance:"
    Int deg = 80
    While deg <= 180
        r += " " + deg + ":" + SeenAt(o, 300.0 * Math.sin(deg), 300.0 * Math.cos(deg))
        deg += 10
    EndWhile
    r += "\n  ahead at 300: " + SeenAt(o, 0.0, 300.0) + "; behind at 150: " + SeenAt(o, 0.0, -150.0) + "\n"
    Gone(o)
    GoCell()
    If Player.IsSneaking()
        Player.StartSneaking()
    EndIf
    return r
EndFunction

; disarm: what decides it (autotest23: our copy at 12 disarmed a level 1
; NPC, not a level 10 one or a level 20 one at 12 or 30). Each case twice
; where cheap, levels 1, 8, 10 and 20, magnitudes 12 and 30.
String Function DisarmCase(ActorBase b, Form weapon, Spell s, Bool logged, String label)
    Actor a = Spawn(b, 0.0, 250.0)
    Actor t = Spawn(InvulnerableBase, 0.0, 450.0)
    t.SetDontMove(true)
    a.AddItem(weapon, 1, true)
    a.EquipItem(weapon, false, true)
    Utility.Wait(1.0)
    a.StartCombat(t)
    Utility.Wait(3.0)
    If logged
        (a as VrmAt21ActorLog).Take()
    EndIf
    String r = "  " + label + " (level " + a.GetLevel() + ", OneHanded " + Num(a.GetActorValue("OneHanded")) + ", TwoHanded " + Num(a.GetActorValue("TwoHanded")) + "): drawn " + a.IsWeaponDrawn() + ", combat " + a.GetCombatState()
    s.Cast(Player, a)
    Utility.Wait(2.0)
    r += "; after: holds " + Id(a.GetEquippedWeapon(false)) + ", carries " + a.GetItemCount(weapon)
    If logged
        r += ", events " + (a as VrmAt21ActorLog).Take()
    EndIf
    a.StopCombat()
    Gone(a)
    Gone(t)
    ObjectReference w = Game.FindClosestReferenceOfTypeFromRef(weapon, CellCenter, 3000.0)
    While w != None
        Gone(w)
        Utility.Wait(0.2)
        w = Game.FindClosestReferenceOfTypeFromRef(weapon, CellCenter, 3000.0)
        If w != None && w.IsDeleted()
            w = None
        EndIf
    EndWhile
    return r + "\n"
EndFunction

String Function TestDisarm()
    String r = "our target-actor Disarm, by script from the player:\n"
    r += DisarmCase(LowLoggedBase, IronSword, OurDisarm, true, "12, level 1")
    r += DisarmCase(LowLoggedBase, IronSword, OurDisarm, true, "12, level 1 again")
    r += DisarmCase(LowLoggedBase, IronGreatsword, OurDisarm, true, "12, level 1, a greatsword")
    r += DisarmCase(Level8LoggedBase, IronSword, OurDisarm, true, "12, level 8")
    r += DisarmCase(LoggedBase, IronSword, OurDisarm, true, "12, level 10")
    r += DisarmCase(LoggedBase, IronSword, OurDisarm, true, "12, level 10 again")
    r += DisarmCase(LoggedBase, IronSword, OurDisarmStrong, true, "30, level 10")
    r += DisarmCase(Level20FighterBase, IronSword, OurDisarmStrong, false, "30, level 20, a sword")
    return r
EndFunction
