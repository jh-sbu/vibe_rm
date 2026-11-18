; death, last in the run, in the test cell: the kill, GetTimeDead twice, and
; last IsKillerObject run on the dead victim. autotest23 and autotest24
; crashed SE in a test that killed Victim and ran that probe at once; here
; the kill is its own test, so a crash in the last one pins it on the probe.

; The Story Manager log the story quests write to (VrmAt21StoryLog).
String StoryLog = ""

Function AddStory(String s)
    StoryLog = StoryLog + s + "@" + Math.Floor(Utility.GetCurrentRealTime() * 100.0) + "; "
EndFunction

Float KillTime = 0.0
Float KillHours = 0.0

Float Function GameHours()
    return Utility.GetCurrentGameTime() * 24.0
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

String Function TestKillVictim()
    GoCell()
    Alpha.SetDontMove(true)
    Victim.SetDontMove(true)
    Alpha.MoveTo(CellCenter, -200.0, -600.0, 0.0, false)
    Victim.MoveTo(CellCenter, 200.0, -600.0, 0.0, false)
    Utility.Wait(1.0)
    Victim.Kill(Alpha)
    KillTime = Now()
    KillHours = GameHours()
    Utility.Wait(1.0)
    return "Victim killed by Alpha: dead " + Victim.IsDead() + ", GetKiller " + Id(Victim.GetKiller()) + " (Alpha " + Id(Alpha) + ")\n"
EndFunction

String Function TimeDeadRead()
    If !Victim.IsDead()
        return "Victim isn't dead: no reading\n"
    EndIf
    Float a = Now() - KillTime
    Float ga = GameHours() - KillHours
    String s = Bisect(Alpha, PS_TimeDead, PE_TimeDead, 0.0, 2000.0, 15)
    return "GetTimeDead " + s + ", read from " + Num(a) + " to " + Num(Now() - KillTime) + " real s (" + Num(ga) + " to " + Num(GameHours() - KillHours) + " game hours) after the kill\n"
EndFunction

; GetTimeDead's unit: real minutes and tenths of a game hour both fitted
; autotest22. Read once, then again after game time has run at timescale
; 100 for 30 s (about 0.8 game hours more than real time gives).
String Function TestTimeDead()
    return TimeDeadRead()
EndFunction

String Function TestTimeDeadLater()
    Float ts = TimeScale.GetValue()
    TimeScale.SetValue(100.0)
    Utility.Wait(30.0)
    TimeScale.SetValue(ts)
    Utility.Wait(1.0)
    return "timescale 100 for 30 s (was " + Num(ts) + "), then: " + TimeDeadRead()
EndFunction

; Expected to crash SE. The game has saved every report before it; if it
; crashes, the run is over and the crash is the reading.
String Function TestKillerObjectDead()
    return "on the dead Victim: IsKillerObject(Alpha's base) " + Probe(Alpha, PS_KillerObjBase, PE_KillerObjBase) + ", (Alpha) " + Probe(Alpha, PS_KillerObjRef, PE_KillerObjRef) + "\n"
EndFunction
