; Outdoors, where the run began (open, flat ground): the charge with both
; NPCs fighting, and Attack on Sight with a guard of the hold's own crime
; faction. Skipped when the run began indoors.

Bool Function Outside()
    Player.MoveTo(StartMarker, 0.0, 0.0, 0.0, true)
    Utility.Wait(5.0)
    return !Player.IsInInterior()
EndFunction

; An actor of base b placed x / y from the start marker.
Actor Function SpawnOut(ActorBase b, Float x, Float y)
    Actor a = StartMarker.PlaceActorAtMe(b, 4, None)
    a.MoveTo(StartMarker, x, y, 0.0, false)
    return a
EndFunction

; A gait: S sprinting, R running, W neither.
String Function Gait(Actor a)
    If a.IsSprinting()
        return "S"
    ElseIf a.IsRunning()
        return "R"
    EndIf
    return "W"
EndFunction

; The pair's distance and both gaits (fighter first) every 0.25 s, four
; samples a line with the time first, until within near or seconds pass.
String Function Chase2(Actor a, Actor b, Float seconds, Float near)
    String r = ""
    Float t0 = Now()
    Int i = 0
    While Now() - t0 < seconds
        If i % 4 == 0
            r += "    " + Num(Now() - t0) + ":"
        EndIf
        Float d = a.GetDistance(b)
        r += " " + Math.Floor(d) + " " + Gait(a) + Gait(b)
        i += 1
        If i % 4 == 0
            r += "\n"
        EndIf
        If d < near
            t0 = -1000.0
        Else
            Utility.Wait(0.25)
        EndIf
    EndWhile
    If i % 4 != 0
        r += "\n"
    EndIf
    return r
EndFunction

; combat movement: autotest20's fighter never engaged a held target that
; didn't fight back. Here two level 15 fighters (swords, 5000 health) fight
; each other, both free; once engaged, the second is moved `far` past the
; first along their line, and both are sampled closing in again.
String Function Charge(Float far)
    If !Outside()
        return "skipped: the run began indoors\n"
    EndIf
    Actor a = SpawnOut(Level15FighterBase, 250.0, 0.0)
    Actor b = SpawnOut(Level15FighterBase, 550.0, 0.0)
    Tough(a)
    Tough(b)
    Utility.Wait(1.5)
    Fight(a, b)
    Float e = Engage(a, b, 10.0)
    String r = "engaged after " + Num(e) + " s at " + Math.Floor(a.GetDistance(b)) + ", states " + a.GetCombatState() + "/" + b.GetCombatState() + ", drawn " + a.IsWeaponDrawn() + "/" + b.IsWeaponDrawn() + ", stamina " + Num(a.GetActorValue("Stamina")) + "/" + Num(b.GetActorValue("Stamina")) + "\n"
    Float dx = b.GetPositionX() - a.GetPositionX()
    Float dy = b.GetPositionY() - a.GetPositionY()
    Float len = Math.Sqrt(dx * dx + dy * dy)
    If len < 1.0
        dx = 1.0
        dy = 0.0
        len = 1.0
    EndIf
    b.MoveTo(a, dx / len * far, dy / len * far, 0.0, false)
    Utility.Wait(0.2)
    r += "second moved: " + Math.Floor(a.GetDistance(b)) + " off, height against the first " + Num(b.GetPositionZ() - a.GetPositionZ()) + "; distance and gaits (first, second):\n"
    r += Chase2(a, b, 16.0, 200.0)
    r += "then states " + a.GetCombatState() + "/" + b.GetCombatState() + ", stamina " + Num(a.GetActorValue("Stamina")) + "/" + Num(b.GetActorValue("Stamina")) + ", health lost " + Num(5000.0 - Health(a)) + "/" + Num(5000.0 - Health(b)) + "\n"
    a.StopCombat()
    b.StopCombat()
    Gone(a)
    Gone(b)
    Utility.Wait(1.0)
    return r
EndFunction

String Function TestCharge1500()
    return Charge(1500.0)
EndFunction

String Function TestCharge2500()
    return Charge(2500.0)
EndFunction

; crime: Attack on Sight in a guard's own hold (autotest24's guard of our
; faction, in a cell with no location, walked up and didn't attack). A
; voiceless IsGuard guard of Whiterun's crime faction (Arrest and Attack on
; Sight), 500 off, the player's Whiterun bounty 1000 for 40 s. Only in
; Whiterun Hold; the bounty is put back after.
String Function TestAttackOnSightHold()
    If !Outside()
        return "skipped: the run began indoors\n"
    EndIf
    If !Player.IsInLocation(WhiterunHold)
        return "skipped: not in Whiterun Hold (location " + Id(Player.GetCurrentLocation()) + ")\n"
    EndIf
    Player.ModActorValue("Health", 1000.0)
    Player.RestoreActorValue("Health", 1000.0)
    Int was = CrimeWhiterun.GetCrimeGold()
    CrimeWhiterun.SetCrimeGold(0)
    Actor g = SpawnOut(HoldGuardBase, 500.0, 0.0)
    Utility.Wait(3.0)
    String r = "in Whiterun Hold (location " + Id(Player.GetCurrentLocation()) + "); a guard (IsGuard " + g.IsGuard() + ", crime faction " + Id(g.GetCrimeFaction()) + "), 500 off; the bounty was " + was + "\nbounty 0: " + Views(g) + "\n"
    CrimeWhiterun.SetCrimeGold(1000)
    Float start = Now()
    While Now() - start < 40.0
        Utility.Wait(5.0)
        r += "  bounty 1000 +" + Num(Now() - start) + " s: " + Views(g) + ", player's health " + Num(Health(Player)) + "\n"
    EndWhile
    CrimeWhiterun.SetCrimeGold(was)
    g.StopCombat()
    Player.StopCombatAlarm()
    Gone(g)
    Player.RestoreActorValue("Health", 2000.0)
    Player.ModActorValue("Health", -1000.0)
    return r
EndFunction
