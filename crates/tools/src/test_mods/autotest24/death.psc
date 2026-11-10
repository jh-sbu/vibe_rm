; death, last in the run and in three tests: autotest23's TimeDead crashed
; SE (likely its timescale of 1000), so a crash here costs nothing else and
; its own save shows how far it got.

Float KillTime = 0.0
Float KillHours = 0.0

Float Function GameHours()
    return Utility.GetCurrentGameTime() * 24.0
EndFunction

; IsKillerObject run on the victim, with a list of the killer's base and of
; the killer's reference.
String Function TestKillerObject()
    Alpha.SetDontMove(true)
    Victim.SetDontMove(true)
    Alpha.MoveTo(CellCenter, -200.0, -600.0, 0.0, false)
    Victim.MoveTo(CellCenter, 200.0, -600.0, 0.0, false)
    Utility.Wait(1.0)
    Victim.Kill(Alpha)
    KillTime = Now()
    KillHours = GameHours()
    Utility.Wait(1.0)
    return "Victim killed by Alpha: GetKiller " + Id(Victim.GetKiller()) + " (Alpha " + Id(Alpha) + "); on the victim, IsKillerObject(Alpha's base) " + Probe(Alpha, PS_KillerObjBase, PE_KillerObjBase) + ", (Alpha) " + Probe(Alpha, PS_KillerObjRef, PE_KillerObjRef) + "\n"
EndFunction

String Function TimeDeadRead()
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
