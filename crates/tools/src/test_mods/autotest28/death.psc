; death, in the test cell: GetTimeDead's unit (autotest27 read whole
; numbers, 0 then 1). Hours struck on the clock since the death, or whole
; real minutes: a victim killed just before the hour strikes reads 1 within
; seconds by the first, after a minute by the second; one killed just after
; the hour, after about 3 real minutes (timescale 20) by the first. No
; IsKillerObject here: run on the dead, it crashes SE (autotest27).

Float Function GameHours()
    return Utility.GetCurrentGameTime() * 24.0
EndFunction

; The fraction of the hour on the clock.
Float Function OfHour()
    Float h = GameHour.GetValue()
    return h - Math.Floor(h)
EndFunction

; Waits (up to 200 s) for the clock to be between lo and hi into its hour.
Function UntilOfHour(Float lo, Float hi)
    Float start = Now()
    While (OfHour() < lo || OfHour() >= hi) && Now() - start < 200.0
        Utility.Wait(0.5)
    EndWhile
EndFunction

; The victim v (placed y from the cell's centre, probe s/e on it) killed
; by Alpha once the clock is between lo and hi into its hour; then
; GetTimeDead >= 1 polled for up to `limit` seconds: when it first holds,
; in real seconds and game hours.
String Function TimeDeadFrom(Actor v, Float y, Spell s, MagicEffect e, Float lo, Float hi, Float limit)
    v.SetDontMove(true)
    v.MoveTo(CellCenter, 200.0, y, 0.0, false)
    Utility.Wait(1.0)
    UntilOfHour(lo, hi)
    Float clock = GameHour.GetValue()
    Float t0 = Now()
    Float g0 = GameHours()
    v.Kill(Alpha)
    String r = "killed at " + Num(clock) + " on the clock (timescale " + Num(TimeScale.GetValue()) + "), dead " + v.IsDead() + "; GetTimeDead >= 1:"
    ProbeValue.SetValue(1.0)
    Float found = -1.0
    While found < 0.0 && Now() - t0 < limit
        Float before = Now() - t0
        If Probe(Alpha, s, e) == "yes"
            found = before
        EndIf
    EndWhile
    If found < 0.0
        return r + " not by " + Num(Now() - t0) + " s (" + Num(GameHours() - g0) + " game hours, the clock " + Num(GameHour.GetValue()) + ")\n"
    EndIf
    r += " first held in the probe from " + Num(found) + " s (" + Num(GameHours() - g0) + " game hours after, the clock " + Num(GameHour.GetValue()) + ")"
    ProbeValue.SetValue(2.0)
    return r + "; >= 2 then " + Probe(Alpha, s, e) + "\n"
EndFunction

String Function TestTimeDeadBeforeHour()
    GoCell()
    Alpha.SetDontMove(true)
    Alpha.MoveTo(CellCenter, -200.0, -600.0, 0.0, false)
    return "Victim, killed 0.9 to 0.95 into the hour: " + TimeDeadFrom(Victim, -600.0, PS_TimeDead, PE_TimeDead, 0.9, 0.95, 100.0)
EndFunction

String Function TestTimeDeadAfterHour()
    return "Gamma, killed 0.02 to 0.1 into the hour: " + TimeDeadFrom(Gamma, -300.0, PS_TimeDeadGamma, PE_TimeDeadGamma, 0.02, 0.1, 220.0)
EndFunction
