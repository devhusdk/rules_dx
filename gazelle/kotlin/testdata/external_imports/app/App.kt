package com.example.app

import com.example.util.Widget
import com.example.util.widget
import kotlin.reflect.full.memberProperties
import kotlinx.coroutines.runBlocking

fun app() = runBlocking { widget() }

fun fields(): Int = Widget::class.memberProperties.size
