Scriptname VrmSaveLoadActorScript extends Actor
; Each test actor's events, with real time and game hours, at most 30.

String ResultEvents = ""
Int Count = 0

Function Log(String what)
    If Count >= 30
        return
    EndIf
    Count += 1
    Float t = Utility.GetCurrentRealTime()
    Float g = Utility.GetCurrentGameTime() * 24.0
    ResultEvents += (Math.Floor(t * 10.0) / 10.0) + " g" + (Math.Floor(g * 1000.0) / 1000.0) + " " + what + ", sit" + GetSitState() + " 3D" + Is3DLoaded() + "\n"
EndFunction

Event OnLoad()
    Log("OnLoad")
EndEvent

Event OnUnload()
    Log("OnUnload")
EndEvent

Event OnCellAttach()
    Log("OnCellAttach")
EndEvent

Event OnPackageStart(Package p)
    Log("OnPackageStart #" + p.GetFormID())
EndEvent

Event OnPackageEnd(Package p)
    Log("OnPackageEnd #" + p.GetFormID())
EndEvent

Event OnPackageChange(Package p)
    Log("OnPackageChange from #" + p.GetFormID())
EndEvent

Event OnSit(ObjectReference f)
    Log("OnSit #" + f.GetFormID())
EndEvent

Event OnGetUp(ObjectReference f)
    Log("OnGetUp #" + f.GetFormID())
EndEvent

Event OnCombatStateChanged(Actor target, Int state)
    Log("OnCombatStateChanged " + state)
EndEvent
