; autotest28's helpers, beside autotest22's base.psc (several are
; autotest27's).

; The Story Manager log the story quests write to (VrmAt21StoryLog).
String StoryLog = ""

Function AddStory(String s)
    StoryLog = StoryLog + s + "@" + Math.Floor(Utility.GetCurrentRealTime() * 100.0) + "; "
EndFunction

; Waits up to t seconds for a to be in combat (state 1) within reach of b;
; the seconds taken, or -1.
Float Function Engage(Actor a, Actor b, Float t)
    Float start = Now()
    While Now() - start < t
        If a.GetCombatState() == 1 && a.GetDistance(b) < 200.0
            return Now() - start
        EndIf
        Utility.Wait(0.25)
    EndWhile
    return -1.0
EndFunction

; Health enough that nobody dies of a test's fight.
Function Tough(Actor a)
    a.ForceActorValue("Health", 5000.0)
    a.RestoreActorValue("Health", 5000.0)
EndFunction

; Deletes every reference of weapon lying in the warehouse.
Function SweepWh(Form weapon)
    ObjectReference w = Game.FindClosestReferenceOfTypeFromRef(weapon, WhCenter, 4000.0)
    While w != None
        Gone(w)
        Utility.Wait(0.2)
        w = Game.FindClosestReferenceOfTypeFromRef(weapon, WhCenter, 4000.0)
        If w != None && w.IsDeleted()
            w = None
        EndIf
    EndWhile
EndFunction

Function Reset()
    Player.StopCombatAlarm()
    Player.MoveTo(WhCenter, -500.0, 0.0, 0.0, false)
    Player.RestoreActorValue("Health", 2000.0)
    Utility.Wait(1.0)
EndFunction

; What an NPC's view of the player is.
String Function Views(Actor a)
    return "hostile " + a.IsHostileToActor(Player) + ", detects " + Player.IsDetectedBy(a) + ", state " + a.GetCombatState() + ", targets the player " + (a.GetCombatTarget() == Player) + ", drawn " + a.IsWeaponDrawn() + ", distance " + Num(a.GetDistance(Player))
EndFunction

; Who a's combat target is, among the player and o.
String Function TargetOf(Actor a, Actor o)
    If a.GetCombatTarget() == o
        return "other"
    ElseIf a.GetCombatTarget() == Player
        return "player"
    ElseIf a.GetCombatTarget() != None
        return Id(a.GetCombatTarget())
    EndIf
    return "none"
EndFunction
