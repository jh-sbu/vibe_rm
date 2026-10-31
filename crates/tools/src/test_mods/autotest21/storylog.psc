Scriptname VrmAt21StoryLog extends Quest
; Attached to a quest the Story Manager starts for one event type: hands the
; event's members to the main script and stops, so the next event of the
; type can start it again.
Quest Property Main Auto

Function Report(String s)
    (Main as VrmAt21Script).AddStory(s)
    Stop()
EndFunction

String Function R(ObjectReference o)
    If o == None
        return "none"
    EndIf
    return "#" + o.GetFormID()
EndFunction

String Function F(Form o)
    If o == None
        return "none"
    EndIf
    return "#" + o.GetFormID()
EndFunction

String Function L(Location o)
    If o == None
        return "none"
    EndIf
    return "#" + o.GetFormID()
EndFunction

Event OnStoryRemoveFromPlayer(ObjectReference akOwner, ObjectReference akItem, Location akLocation, Form akItemBase, Int aiRemoveType)
    Report("REMP owner " + R(akOwner) + " item " + R(akItem) + " base " + F(akItemBase) + " type " + aiRemoveType)
EndEvent

Event OnStoryAddToPlayer(ObjectReference akOwner, ObjectReference akContainer, Location akLocation, Form akItemBase, Int aiAcquireType)
    Report("AIPL owner " + R(akOwner) + " container " + R(akContainer) + " base " + F(akItemBase) + " type " + aiAcquireType)
EndEvent

Event OnStoryKillActor(ObjectReference akVictim, ObjectReference akKiller, Location akLocation, Int aiCrimeStatus, Int aiRelationshipRank)
    Report("KILL victim " + R(akVictim) + " killer " + R(akKiller) + " crime " + aiCrimeStatus + " rank " + aiRelationshipRank)
EndEvent

Event OnStoryAssaultActor(ObjectReference akVictim, ObjectReference akAttacker, Location akLocation, Int aiCrime)
    Report("ASSU victim " + R(akVictim) + " attacker " + R(akAttacker) + " crime " + aiCrime)
EndEvent

Event OnStoryCastMagic(ObjectReference akCastingActor, ObjectReference akSpellTarget, Location akLocation, Form akSpell)
    Report("CAST caster " + R(akCastingActor) + " target " + R(akSpellTarget) + " spell " + F(akSpell))
EndEvent

Event OnStoryCrimeGold(ObjectReference akVictim, ObjectReference akCriminal, Form akFaction, Int aiGoldAmount, Int aiCrime)
    Report("ADCR victim " + R(akVictim) + " criminal " + R(akCriminal) + " faction " + F(akFaction) + " gold " + aiGoldAmount + " crime " + aiCrime)
EndEvent

Event OnStoryIncreaseSkill(String asSkill)
    Report("SKIL " + asSkill)
EndEvent

Event OnStoryNewVoicePower(ObjectReference akActor, Form akVoicePower)
    Report("NVPE actor " + R(akActor) + " power " + F(akVoicePower))
EndEvent

Event OnStoryRelationshipChange(ObjectReference akActor1, ObjectReference akActor2, Int aiOldRelationship, Int aiNewRelationship)
    Report("CHRR " + R(akActor1) + " " + R(akActor2) + " " + aiOldRelationship + ">" + aiNewRelationship)
EndEvent
