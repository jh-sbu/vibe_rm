Scriptname VrmAt21RefLog extends ObjectReference
; Logs what happens to the reference it is attached to, each event with the
; real time.
String Log = ""
Int Marker = 7

Function Add(String s)
    Log = Log + s + "@" + Math.Floor(Utility.GetCurrentRealTime() * 100.0) + " "
EndFunction

String Function Take()
    String s = Log
    Log = ""
    return s
EndFunction

Event OnOpen(ObjectReference akActionRef)
    Add("open")
EndEvent

Event OnClose(ObjectReference akActionRef)
    Add("close")
EndEvent

Event OnActivate(ObjectReference akActionRef)
    Add("activate")
EndEvent

Event OnHit(ObjectReference akAggressor, Form akSource, Projectile akProjectile, Bool abPowerAttack, Bool abSneakAttack, Bool abBashAttack, Bool abHitBlocked)
    Add("hit")
EndEvent

Event OnDestructionStageChanged(Int aiOldStage, Int aiCurrentStage)
    Add("stage" + aiOldStage + ">" + aiCurrentStage)
EndEvent

Event OnTriggerEnter(ObjectReference akActionRef)
    Add("enter")
EndEvent

Event OnTrigger(ObjectReference akActionRef)
    Add("trigger")
EndEvent

Event OnTriggerLeave(ObjectReference akActionRef)
    Add("leave")
EndEvent

Event OnLoad()
    Add("load")
EndEvent

Event OnCellLoad()
    Add("cellload")
EndEvent

Event OnCellAttach()
    Add("attach")
EndEvent

Event OnReset()
    Add("reset")
EndEvent

Event OnContainerChanged(ObjectReference akNewContainer, ObjectReference akOldContainer)
    Add("container")
EndEvent
