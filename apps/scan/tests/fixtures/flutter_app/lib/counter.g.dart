// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'counter.dart';

// **************************************************************************
// JsonSerializableGenerator
// **************************************************************************

class CounterModel {
  CounterModel(this.value);

  final int value;

  static CounterModel fromJson(Map<String, dynamic> json) {
    return CounterModel(json['value'] as int);
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        'value': value,
      };
}
